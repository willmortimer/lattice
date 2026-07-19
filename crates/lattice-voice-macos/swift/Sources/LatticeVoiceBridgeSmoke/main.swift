@preconcurrency import AVFoundation
import Darwin
import Foundation
import LatticeVoiceBridge
import LatticeVoiceBridgeC

/// Fixture smoke against the C ABI. Requires a prepared Unified model cache.
///
/// ```sh
/// export LATTICE_VOICE_MODEL_CACHE=/path/to/Models
/// swift run -c release lattice-voice-bridge-smoke /path/to/fixture.wav
/// ```
@main
enum LatticeVoiceBridgeSmoke {
    static func main() {
        let args = CommandLine.arguments
        guard args.count >= 2 else {
            fputs(
                """
                Usage: lattice-voice-bridge-smoke <fixture.wav>
                Set LATTICE_VOICE_MODEL_CACHE to a FluidAudio Models directory
                (e.g. research/voice-m0-fluidaudio/.cache/Models).

                """,
                stderr
            )
            exit(2)
        }

        let fixture = URL(fileURLWithPath: args[1])
        guard FileManager.default.fileExists(atPath: fixture.path) else {
            fputs("Missing fixture: \(fixture.path)\n", stderr)
            exit(2)
        }

        let abi = lattice_voice_bridge_abi_version()
        print("ABI version: \(abi)")
        guard abi == LatticeVoiceBridge.LATTICE_VOICE_BRIDGE_ABI_VERSION else {
            fputs("ABI mismatch\n", stderr)
            exit(1)
        }

        let cacheDir = ProcessInfo.processInfo.environment["LATTICE_VOICE_MODEL_CACHE"]
        let cacheCString = cacheDir.map { strdup($0) } ?? nil
        defer {
            if let cacheCString { free(cacheCString) }
        }

        var engine: UInt64 = 0
        let createRc = lattice_voice_engine_create(
            model_cache_dir: cacheCString,
            out_engine: &engine
        )
        guard createRc == 0, engine != 0 else {
            fputs("engine_create failed: \(createRc)\n", stderr)
            exit(1)
        }
        defer { lattice_voice_engine_destroy(engine: engine) }

        print("Preparing Unified streaming checkpoint…")
        let prepareRc = lattice_voice_engine_prepare(engine: engine)
        guard prepareRc == 0 else {
            fputs("engine_prepare failed: \(prepareRc)\n", stderr)
            exit(1)
        }
        print("Engine prepared.")

        let sink = EventSink()
        var session: UInt64 = 0
        let startRc = lattice_voice_session_start(
            engine: engine,
            callback: EventSink.callback,
            context: Unmanaged.passUnretained(sink).toOpaque(),
            out_session: &session
        )
        guard startRc == 0, session != 0 else {
            fputs("session_start failed: \(startRc)\n", stderr)
            exit(1)
        }
        defer { lattice_voice_session_destroy(session: session) }

        do {
            let samples = try loadMonoFloat32(url: fixture)
            print("Loaded \(samples.count) samples from \(fixture.lastPathComponent)")

            let chunk = 5120  // 320 ms @ 16 kHz
            var offset = 0
            while offset < samples.count {
                let end = min(offset + chunk, samples.count)
                let slice = Array(samples[offset..<end])
                let rc = slice.withUnsafeBufferPointer { buf -> Int32 in
                    lattice_voice_session_push_audio(
                        session: session,
                        samples: buf.baseAddress,
                        sample_count: buf.count
                    )
                }
                if rc != 0 {
                    fputs("push_audio failed: \(rc)\n", stderr)
                    exit(1)
                }
                offset = end
            }

            let finishRc = lattice_voice_session_finish_utterance(session: session)
            guard finishRc == 0 else {
                fputs("finish_utterance failed: \(finishRc)\n", stderr)
                exit(1)
            }
        } catch {
            fputs("ERROR: \(error)\n", stderr)
            exit(1)
        }

        print("Partials: \(sink.partialCount)")
        print("Final: \"\(sink.finalText ?? "")\"")
        if let err = sink.errorText {
            fputs("Bridge error event: \(err)\n", stderr)
            exit(1)
        }
        guard let finalText = sink.finalText, !finalText.isEmpty else {
            fputs("No final transcript\n", stderr)
            exit(1)
        }
        print("DONE")
    }

    static func loadMonoFloat32(url: URL) throws -> [Float] {
        let file = try AVAudioFile(forReading: url)
        let format = file.processingFormat
        let frameCount = AVAudioFrameCount(file.length)
        guard
            let buffer = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: frameCount)
        else {
            throw SmokeError.buffer
        }
        try file.read(into: buffer)

        if format.commonFormat == .pcmFormatFloat32, format.channelCount == 1,
            format.sampleRate == 16_000, let channel = buffer.floatChannelData?[0]
        {
            return Array(UnsafeBufferPointer(start: channel, count: Int(buffer.frameLength)))
        }

        guard
            let target = AVAudioFormat(
                commonFormat: .pcmFormatFloat32,
                sampleRate: 16_000,
                channels: 1,
                interleaved: false
            )
        else {
            throw SmokeError.format
        }
        guard let converter = AVAudioConverter(from: format, to: target) else {
            throw SmokeError.format
        }
        let ratio = 16_000 / format.sampleRate
        let outCapacity = AVAudioFrameCount(Double(buffer.frameLength) * ratio) + 32
        guard let converted = AVAudioPCMBuffer(pcmFormat: target, frameCapacity: outCapacity)
        else {
            throw SmokeError.buffer
        }
        var error: NSError?
        let inputBlock: AVAudioConverterInputBlock = { _, outStatus in
            outStatus.pointee = .haveData
            return buffer
        }
        converter.convert(to: converted, error: &error, withInputFrom: inputBlock)
        if let error { throw error }
        guard let channel = converted.floatChannelData?[0] else {
            throw SmokeError.buffer
        }
        return Array(UnsafeBufferPointer(start: channel, count: Int(converted.frameLength)))
    }
}

enum SmokeError: Error {
    case buffer
    case format
}

final class EventSink: @unchecked Sendable {
    private let lock = NSLock()
    private(set) var partialCount = 0
    private(set) var finalText: String?
    private(set) var errorText: String?

    static let callback: lattice_voice_event_callback = { eventPtr, context in
        guard let context, let eventPtr else { return }
        let sink = Unmanaged<EventSink>.fromOpaque(context).takeUnretainedValue()
        let event = eventPtr.pointee
        let text: String = {
            guard let cText = event.text else { return "" }
            return String(cString: cText)
        }()
        sink.lock.lock()
        defer { sink.lock.unlock() }
        switch event.kind {
        case LATTICE_VOICE_EVENT_PARTIAL:
            sink.partialCount += 1
            fputs("  [partial] \(text)\n", stdout)
        case LATTICE_VOICE_EVENT_STABLE:
            fputs("  [stable] \(text)\n", stdout)
        case LATTICE_VOICE_EVENT_FINAL:
            sink.finalText = text
            fputs("  [final] \(text)\n", stdout)
        case LATTICE_VOICE_EVENT_ERROR:
            sink.errorText = text
            fputs("  [error] \(text)\n", stderr)
        default:
            fputs("  [unknown event \(Int(event.kind.rawValue))] \(text)\n", stderr)
        }
    }
}
