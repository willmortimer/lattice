import AVFoundation
import CoreML
import FluidAudio
import Foundation
import os

/// Milestone 0 / Task U research spike: FluidAudio Parakeet paths on one fixture.
/// Modes: `eou-tdt` (M0 default) and `unified` (Task U production decision).

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
        let mode = parseMode()
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
        print("Mode: \(mode.rawValue)")
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

        switch mode {
        case .eouTdt:
            try await runEouTdt(samples: samples, modelsRoot: modelsRoot)
        case .unified:
            try await runUnified(samples: samples, modelsRoot: modelsRoot)
        }
    }

    // MARK: - Modes

    enum Mode: String {
        case eouTdt = "eou-tdt"
        case unified = "unified"
    }

    static func parseMode() -> Mode {
        let args = CommandLine.arguments
        if let idx = args.firstIndex(of: "--mode"), idx + 1 < args.count {
            let value = args[idx + 1]
            switch value {
            case Mode.eouTdt.rawValue:
                return .eouTdt
            case Mode.unified.rawValue:
                return .unified
            default:
                fputs(
                    "Unknown --mode \(value). Use eou-tdt or unified.\n",
                    stderr
                )
                exit(2)
            }
        }
        // Bare `unified` / `eou-tdt` after executable name.
        if args.count >= 2 {
            switch args[1] {
            case Mode.unified.rawValue:
                return .unified
            case Mode.eouTdt.rawValue:
                return .eouTdt
            default:
                break
            }
        }
        return .eouTdt
    }

    // MARK: - EOU + TDT (M0)

    static func runEouTdt(samples: [Float], modelsRoot: URL) async throws {
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
        try await streaming.loadModels(to: modelsRoot)
        let streamingLoadMs = elapsedMs(from: streamingLoadStart)
        print("Streaming model load: \(fmt(streamingLoadMs)) ms")

        await streaming.reset()
        let streamingStart = ContinuousClock.now
        streamingTimings.markStart(at: streamingStart)

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
        print(
            "Offline decode: \(fmt(offlineMs)) ms (model reports processingTime=\(fmt(offlineResult.processingTime * 1000)) ms)"
        )
        print("Offline confidence: \(offlineResult.confidence)")
        print("")

        let offlineBeatsStreaming = normalize(offlineResult.text) != normalize(streamingFinal)
            && !offlineResult.text.isEmpty
        print("=== Comparison ===")
        print("Streaming == offline (normalized)? \(normalize(streamingFinal) == normalize(offlineResult.text))")
        print("Offline differs from streaming final? \(offlineBeatsStreaming)")
        print("")

        print("=== Machine-readable timings ===")
        print("MODE=eou-tdt")
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

    // MARK: - Unified (Task U)

    static func runUnified(samples: [Float], modelsRoot: URL) async throws {
        // Lowest-latency Unified tier for first-partial rubric fairness vs EOU 160ms.
        let variant = StreamingModelVariant.parakeetUnified320ms
        guard let unifiedConfig = variant.unifiedConfig else {
            throw SpikeError.config
        }

        print(
            "--- Streaming path: StreamingUnifiedAsrManager (\(variant.rawValue) / \(unifiedConfig.contextSuffix)) ---"
        )
        print(
            "Theoretical latency: \(unifiedConfig.latencyMs) ms; chunkSamples=\(unifiedConfig.chunkSamples)"
        )

        let streamingTimings = TimingBox()
        let partialEvents = EventBox()

        let streaming = StreamingUnifiedAsrManager(
            config: unifiedConfig,
            encoderPrecision: .int8
        )

        await streaming.setPartialTranscriptCallback { text in
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

        let streamingLoadStart = ContinuousClock.now
        try await streaming.loadModels(to: modelsRoot)
        let streamingLoadMs = elapsedMs(from: streamingLoadStart)
        print("Unified streaming model load: \(fmt(streamingLoadMs)) ms")

        // Disk footprint of the streaming checkpoint family (same repo folder).
        let unifiedCache = modelsRoot.appendingPathComponent(
            Repo.parakeetUnified.folderName, isDirectory: true)
        let streamingCacheBytes = directoryByteSize(unifiedCache)
        print(
            "Unified cache after streaming load: \(fmtBytes(streamingCacheBytes)) at \(unifiedCache.path)"
        )

        try await streaming.reset()
        let streamingStart = ContinuousClock.now
        streamingTimings.markStart(at: streamingStart)

        // Feed in chunk-sized pieces; first window needs chunk+right samples.
        let feedChunk = unifiedConfig.chunkSamples
        var offset = 0
        while offset < samples.count {
            let end = min(offset + feedChunk, samples.count)
            let chunk = Array(samples[offset..<end])
            let buffer = try makePCMBuffer(samples: chunk, sampleRate: 16_000)
            try await streaming.appendAudio(buffer)
            try await streaming.processBufferedAudio()
            offset = end
        }

        let streamingFinal = try await streaming.finish()
        let streamingFinalizeMs = elapsedMs(from: streamingStart)
        streamingTimings.markFinalize(at: ContinuousClock.now)

        let firstPartialMs = streamingTimings.firstPartialMs()
        print("Unified streaming final text: \"\(streamingFinal)\"")
        print("First partial latency: \(firstPartialMs.map(fmt) ?? "n/a") ms")
        print("Streaming finalization (start→finish): \(fmt(streamingFinalizeMs)) ms")
        print("Partial callbacks: \(partialEvents.count)")
        print("")

        // Authoritative offline final from the same HuggingFace repo / shared decoder+joint,
        // but a distinct full-attention encoder export (second large encoder on disk).
        print("--- Offline path: UnifiedAsrManager (parakeet-unified offline 15s) ---")
        let offline = UnifiedAsrManager(encoderPrecision: .int8)
        let offlineLoadStart = ContinuousClock.now
        try await offline.loadModels(to: modelsRoot)
        let offlineLoadMs = elapsedMs(from: offlineLoadStart)
        print("Unified offline model download+load: \(fmt(offlineLoadMs)) ms")

        let cacheAfterOffline = directoryByteSize(unifiedCache)
        print(
            "Unified cache after offline load: \(fmtBytes(cacheAfterOffline)) (delta \(fmtBytes(max(0, cacheAfterOffline - streamingCacheBytes))))"
        )

        let offlineStart = ContinuousClock.now
        let offlineText = try await offline.transcribe(samples)
        let offlineMs = elapsedMs(from: offlineStart)
        print("Unified offline text: \"\(offlineText)\"")
        print("Unified offline decode: \(fmt(offlineMs)) ms")
        print("")

        // Dual-path session model: streaming manager alone can finish without the offline encoder.
        let dualPathSameManager =
            "StreamingUnifiedAsrManager.finish() is authoritative final from the loaded streaming checkpoint; UnifiedAsrManager offline encoder is optional and a second multi-hundred-MB encoder export in the same HF repo."
        print("=== Dual-path / memory notes ===")
        print(dualPathSameManager)
        print(
            "Streaming-only finals usable without loading offline encoder? YES (measured streaming final above)"
        )
        print("")

        print("=== Technical token quick check ===")
        printTechnicalTokenCheck(label: "unified_streaming_final", text: streamingFinal)
        printTechnicalTokenCheck(label: "unified_offline_final", text: offlineText)
        print("")

        print("=== Machine-readable timings ===")
        print("MODE=unified")
        print("UNIFIED_VARIANT=\(variant.rawValue)")
        print("UNIFIED_CONTEXT=\(unifiedConfig.contextSuffix)")
        print("STREAMING_LOAD_MS=\(fmt(streamingLoadMs))")
        print("FIRST_PARTIAL_MS=\(firstPartialMs.map(fmt) ?? "n/a")")
        print("STREAMING_FINALIZE_MS=\(fmt(streamingFinalizeMs))")
        print("OFFLINE_LOAD_MS=\(fmt(offlineLoadMs))")
        print("OFFLINE_DECODE_MS=\(fmt(offlineMs))")
        print("STREAMING_TEXT=\(streamingFinal)")
        print("OFFLINE_TEXT=\(offlineText)")
        print("PARTIAL_COUNT=\(partialEvents.count)")
        print("CACHE_AFTER_STREAMING_BYTES=\(streamingCacheBytes)")
        print("CACHE_AFTER_OFFLINE_BYTES=\(cacheAfterOffline)")
        print("SAMPLE_FORMAT=Float32@16000mono")
        print("DONE")
    }

    static func printTechnicalTokenCheck(label: String, text: String) {
        let camelPass = text.contains("AsrManager") || text.contains("ASR Manager")
            || text.contains("Asr Manager")
        let pathPass = text.contains("/Users/will/Developer/lattice")
            || text.contains("Users/will/Developer/lattice")
        print("\(label): camelCase_like=\(camelPass ? "pass" : "fail") path_like=\(pathPass ? "pass" : "fail")")
        print("  text=\(text)")
    }

    // MARK: - Helpers

    static func resolvePackageRoot() -> URL {
        let cwd = URL(fileURLWithPath: FileManager.default.currentDirectoryPath, isDirectory: true)
        let marker = cwd.appendingPathComponent("Package.swift")
        if FileManager.default.fileExists(atPath: marker.path) {
            return cwd
        }
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

    static func fmtBytes(_ bytes: UInt64) -> String {
        let mb = Double(bytes) / 1_000_000.0
        return String(format: "%.1f MB", mb)
    }

    static func directoryByteSize(_ url: URL) -> UInt64 {
        let fm = FileManager.default
        guard let enumerator = fm.enumerator(
            at: url,
            includingPropertiesForKeys: [.fileSizeKey, .isRegularFileKey],
            options: [.skipsHiddenFiles]
        ) else {
            return 0
        }
        var total: UInt64 = 0
        for case let fileURL as URL in enumerator {
            guard
                let values = try? fileURL.resourceValues(forKeys: [.fileSizeKey, .isRegularFileKey]),
                values.isRegularFile == true,
                let size = values.fileSize
            else {
                continue
            }
            total += UInt64(size)
        }
        return total
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
    case config

    var description: String {
        switch self {
        case .audioFormat: return "Failed to create Float32 mono audio format"
        case .bufferAlloc: return "Failed to allocate AVAudioPCMBuffer"
        case .config: return "Missing UnifiedConfig for selected variant"
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
