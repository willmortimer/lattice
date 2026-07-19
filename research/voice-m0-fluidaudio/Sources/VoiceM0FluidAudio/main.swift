import AVFoundation
import CoreML
import FluidAudio
import Foundation
import os

/// Milestone 0 research spike: FluidAudio Parakeet EOU streaming + TDT v2 offline
/// on one spoken-English fixture. Models download into `.cache/` (gitignored).

@main
enum VoiceM0FluidAudio {
    static func main() async {
        do {
            try await run()
        } catch {
            fputs("ERROR: \(error)\n", stderr)
            exit(1)
        }
    }

    static func run() async throws {
        let packageRoot = resolvePackageRoot()
        let fixtureURL = packageRoot
            .appendingPathComponent("Fixtures/technical-dictation-16k-mono.wav")
        let cacheRoot = packageRoot.appendingPathComponent(".cache", isDirectory: true)
        let modelsRoot = cacheRoot.appendingPathComponent("Models", isDirectory: true)
        try FileManager.default.createDirectory(at: modelsRoot, withIntermediateDirectories: true)

        guard FileManager.default.fileExists(atPath: fixtureURL.path) else {
            fputs(
                """
                Missing fixture at \(fixtureURL.path)
                Run: ./scripts/generate-fixture.sh
                """,
                stderr
            )
            exit(2)
        }

        print("=== Voice M0 FluidAudio spike ===")
        print("FluidAudio pin: 0.15.5 (tag)")
        print("Package root: \(packageRoot.path)")
        print("Fixture: \(fixtureURL.path)")
        print("Model cache: \(modelsRoot.path)")
        print("Host: \(hostSummary())")
        print("")

        let converter = AudioConverter()
        let samples = try converter.resampleAudioFile(fixtureURL)
        let audioDurationSec = Double(samples.count) / 16_000.0
        print(
            "Audio: \(samples.count) samples, \(String(format: "%.3f", audioDurationSec))s, expected Float32 @ 16 kHz mono"
        )
        print("")

        // --- Streaming: Parakeet realtime EOU 120M (160ms chunks) ---
        print("--- Streaming path: StreamingEouAsrManager (parakeet-realtime-eou-120m-coreml / 160ms) ---")
        let streamingTimings = TimingBox()
        let eouEvents = EventBox()
        let partialEvents = EventBox()

        let streaming = StreamingEouAsrManager(
            chunkSize: .ms160,
            eouDebounceMs: 1280
        )

        await streaming.setPartialCallback { text in
            let now = ContinuousClock.now
            let isFirst = partialEvents.record(text: text, at: now)
            let thread = Thread.isMainThread ? "main" : "background"
            if isFirst {
                streamingTimings.markFirstPartial(at: now)
                print("  [partial#1 @ \(thread)] \(text)")
            } else {
                print("  [partial @ \(thread)] \(text)")
            }
        }
        await streaming.setEouCallback { text in
            let thread = Thread.isMainThread ? "main" : "background"
            eouEvents.record(text: text, at: ContinuousClock.now)
            print("  [eou @ \(thread)] \(text)")
        }

        let streamingLoadStart = ContinuousClock.now
        // `to:` is the Models root; FluidAudio nests parakeet-eou-streaming/<chunk>/ under it.
        try await streaming.loadModels(to: modelsRoot)
        let streamingLoadMs = elapsedMs(from: streamingLoadStart)
        print("Streaming model load: \(fmt(streamingLoadMs)) ms")

        await streaming.reset()
        let streamingStart = ContinuousClock.now
        streamingTimings.markStart(at: streamingStart)

        // Feed in ~160 ms chunks to exercise the streaming path (not one giant buffer).
        let chunkSamples = 2560  // 160 ms @ 16 kHz
        var offset = 0
        while offset < samples.count {
            let end = min(offset + chunkSamples, samples.count)
            let chunk = Array(samples[offset..<end])
            let buffer = try makePCMBuffer(samples: chunk, sampleRate: 16_000)
            _ = try await streaming.process(audioBuffer: buffer)
            offset = end
        }

        let streamingFinal = try await streaming.finish()
        let streamingFinalizeMs = elapsedMs(from: streamingStart)
        streamingTimings.markFinalize(at: ContinuousClock.now)

        let firstPartialMs = streamingTimings.firstPartialMs()
        print("Streaming final text: \"\(streamingFinal)\"")
        print("First partial latency: \(firstPartialMs.map(fmt) ?? "n/a") ms")
        print("Streaming finalization (start→finish): \(fmt(streamingFinalizeMs)) ms")
        print("Partial callbacks: \(partialEvents.count); EOU callbacks: \(eouEvents.count)")
        print("EOU detected flag: \(await streaming.eouDetected)")
        print("")

        // --- Offline: Parakeet TDT English v2 ---
        print("--- Offline path: AsrManager + AsrModels v2 (parakeet-tdt-0.6b-v2-coreml) ---")
        let offlineLoadStart = ContinuousClock.now
        let v2Dir = modelsRoot.appendingPathComponent("parakeet-tdt-0.6b-v2", isDirectory: true)
        let asrModels = try await AsrModels.downloadAndLoad(to: v2Dir, version: .v2)
        let offlineLoadMs = elapsedMs(from: offlineLoadStart)
        print("Offline model download+load: \(fmt(offlineLoadMs)) ms")

        let asr = AsrManager(config: .default)
        try await asr.loadModels(asrModels)

        var decoderState = TdtDecoderState.make(decoderLayers: await asr.decoderLayerCount)
        let offlineStart = ContinuousClock.now
        let offlineResult = try await asr.transcribe(
            samples,
            decoderState: &decoderState
        )
        let offlineMs = elapsedMs(from: offlineStart)
        print("Offline text: \"\(offlineResult.text)\"")
        print("Offline decode: \(fmt(offlineMs)) ms (model reports processingTime=\(fmt(offlineResult.processingTime * 1000)) ms)")
        print("Offline confidence: \(offlineResult.confidence)")
        print("")

        let offlineBeatsStreaming = normalize(offlineResult.text) != normalize(streamingFinal)
            && !offlineResult.text.isEmpty
        print("=== Comparison ===")
        print("Streaming == offline (normalized)? \(normalize(streamingFinal) == normalize(offlineResult.text))")
        print("Offline differs from streaming final? \(offlineBeatsStreaming)")
        print("")

        print("=== Machine-readable timings ===")
        print("STREAMING_LOAD_MS=\(fmt(streamingLoadMs))")
        print("FIRST_PARTIAL_MS=\(firstPartialMs.map(fmt) ?? "n/a")")
        print("STREAMING_FINALIZE_MS=\(fmt(streamingFinalizeMs))")
        print("OFFLINE_LOAD_MS=\(fmt(offlineLoadMs))")
        print("OFFLINE_DECODE_MS=\(fmt(offlineMs))")
        print("STREAMING_TEXT=\(streamingFinal)")
        print("OFFLINE_TEXT=\(offlineResult.text)")
        print("PARTIAL_COUNT=\(partialEvents.count)")
        print("EOU_COUNT=\(eouEvents.count)")
        print("SAMPLE_FORMAT=Float32@16000mono")
        print("DONE")
    }

    // MARK: - Helpers

    static func resolvePackageRoot() -> URL {
        // Prefer cwd when invoked via `swift run` from the package directory.
        let cwd = URL(fileURLWithPath: FileManager.default.currentDirectoryPath, isDirectory: true)
        let marker = cwd.appendingPathComponent("Package.swift")
        if FileManager.default.fileExists(atPath: marker.path) {
            return cwd
        }
        // Fall back to executable-relative walk (release installs).
        let exe = URL(fileURLWithPath: CommandLine.arguments[0]).standardizedFileURL
        var dir = exe.deletingLastPathComponent()
        for _ in 0..<6 {
            if FileManager.default.fileExists(atPath: dir.appendingPathComponent("Package.swift").path) {
                return dir
            }
            dir = dir.deletingLastPathComponent()
        }
        return cwd
    }

    static func hostSummary() -> String {
        var sys = utsname()
        uname(&sys)
        let machine = withUnsafePointer(to: &sys.machine) {
            $0.withMemoryRebound(to: CChar.self, capacity: 1) { String(cString: $0) }
        }
        let ver = ProcessInfo.processInfo.operatingSystemVersionString
        return "\(machine) / \(ver) / \(ProcessInfo.processInfo.processorCount) cores"
    }

    static func makePCMBuffer(samples: [Float], sampleRate: Double) throws -> AVAudioPCMBuffer {
        guard
            let format = AVAudioFormat(
                commonFormat: .pcmFormatFloat32,
                sampleRate: sampleRate,
                channels: 1,
                interleaved: false
            )
        else {
            throw SpikeError.audioFormat
        }
        guard let buffer = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: AVAudioFrameCount(samples.count))
        else {
            throw SpikeError.bufferAlloc
        }
        buffer.frameLength = AVAudioFrameCount(samples.count)
        guard let channel = buffer.floatChannelData?[0] else {
            throw SpikeError.bufferAlloc
        }
        samples.withUnsafeBufferPointer { src in
            channel.update(from: src.baseAddress!, count: samples.count)
        }
        return buffer
    }

    static func elapsedMs(from start: ContinuousClock.Instant) -> Double {
        let duration = ContinuousClock.now - start
        return Double(duration.components.seconds) * 1000.0
            + Double(duration.components.attoseconds) / 1e15
    }

    static func fmt(_ ms: Double) -> String {
        String(format: "%.1f", ms)
    }

    static func normalize(_ text: String) -> String {
        text.lowercased()
            .components(separatedBy: .whitespacesAndNewlines)
            .filter { !$0.isEmpty }
            .joined(separator: " ")
    }
}

enum SpikeError: Error, CustomStringConvertible {
    case audioFormat
    case bufferAlloc

    var description: String {
        switch self {
        case .audioFormat: return "Failed to create Float32 mono audio format"
        case .bufferAlloc: return "Failed to allocate AVAudioPCMBuffer"
        }
    }
}

/// Thread-safe timing marks for callbacks that may fire off the main actor.
final class TimingBox: @unchecked Sendable {
    private let lock = OSAllocatedUnfairLock()
    private var start: ContinuousClock.Instant?
    private var firstPartial: ContinuousClock.Instant?
    private var finalize: ContinuousClock.Instant?

    func markStart(at instant: ContinuousClock.Instant) {
        lock.withLock { start = instant }
    }

    func markFirstPartial(at instant: ContinuousClock.Instant) {
        lock.withLock {
            if firstPartial == nil {
                firstPartial = instant
            }
        }
    }

    func markFinalize(at instant: ContinuousClock.Instant) {
        lock.withLock { finalize = instant }
    }

    func firstPartialMs() -> Double? {
        lock.withLock {
            guard let start, let firstPartial else { return nil }
            let duration = firstPartial - start
            return Double(duration.components.seconds) * 1000.0
                + Double(duration.components.attoseconds) / 1e15
        }
    }
}

final class EventBox: @unchecked Sendable {
    private let lock = OSAllocatedUnfairLock()
    private var texts: [String] = []

    @discardableResult
    func record(text: String, at _: ContinuousClock.Instant) -> Bool {
        lock.withLock {
            let first = texts.isEmpty
            texts.append(text)
            return first
        }
    }

    var count: Int {
        lock.withLock { texts.count }
    }
}
