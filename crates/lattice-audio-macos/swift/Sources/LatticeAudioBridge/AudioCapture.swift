@preconcurrency import AVFoundation
import Darwin
import Foundation
import LatticeAudioBridgeC
import os

private let canonicalSampleRate: Double = 16_000
private let defaultPreRollMs: UInt32 = 300
/// Emit ~20 ms frames (320 samples @ 16 kHz) to match the capture doc.
private let samplesPerEmitFrame = 320

/// Owns AVAudioEngine input tap + AVAudioConverter → 16 kHz mono Float32.
final class AudioCapture: @unchecked Sendable {
    let id: UInt64
    let preRollMs: UInt32
    let enableDiagnostics: Bool

    private let engine = AVAudioEngine()
    private var converter: AVAudioConverter?
    private var canonicalFormat: AVAudioFormat?
    private let preRoll: SampleRingBuffer
    private let stateLock = NSLock()

    private var armed = false
    private var streaming = false
    private var callback: lattice_audio_event_callback?
    private var context: UnsafeMutableRawPointer?
    private var nextSequence: UInt64 = 0
    private var emitScratch = [Float]()
    private var convertScratch = [Float]()

    init(id: UInt64, preRollMs: UInt32, enableDiagnostics: Bool) {
        self.id = id
        self.preRollMs = preRollMs == 0 ? defaultPreRollMs : preRollMs
        self.enableDiagnostics = enableDiagnostics
        let capacity = Int(canonicalSampleRate * Double(self.preRollMs) / 1000.0)
        self.preRoll = SampleRingBuffer(capacity: capacity)
    }

    func arm() throws {
        stateLock.lock()
        if streaming {
            stateLock.unlock()
            throw BridgeFailure.alreadyRunning
        }
        stateLock.unlock()

        try ensurePermission()
        try installTapIfNeeded()
        try startEngineIfNeeded()

        stateLock.lock()
        armed = true
        streaming = false
        preRoll.clear()
        emitScratch.removeAll(keepingCapacity: true)
        stateLock.unlock()
    }

    func start(
        callback: lattice_audio_event_callback?,
        context: UnsafeMutableRawPointer?
    ) throws {
        guard let callback else {
            throw BridgeFailure.invalidArgument("callback is null")
        }

        stateLock.lock()
        if streaming {
            stateLock.unlock()
            throw BridgeFailure.alreadyRunning
        }
        stateLock.unlock()

        try ensurePermission()
        try installTapIfNeeded()
        try startEngineIfNeeded()

        let startedAt = monotonicNanos()
        let preRollSamples: [Float]
        stateLock.lock()
        self.callback = callback
        self.context = context
        self.streaming = true
        self.armed = false
        preRollSamples = preRoll.drain()
        nextSequence = 0
        emitScratch.removeAll(keepingCapacity: true)
        stateLock.unlock()

        emitStarted(capturedAt: startedAt)
        if !preRollSamples.isEmpty {
            emitPcmFrame(preRollSamples, capturedAt: startedAt)
        }
    }

    func stop() throws {
        stateLock.lock()
        let wasActive = streaming || armed
        streaming = false
        armed = false
        let cb = callback
        let ctx = context
        callback = nil
        context = nil
        preRoll.clear()
        emitScratch.removeAll(keepingCapacity: true)
        stateLock.unlock()

        engine.inputNode.removeTap(onBus: 0)
        if engine.isRunning {
            engine.stop()
        }
        converter = nil
        canonicalFormat = nil

        guard wasActive else {
            throw BridgeFailure.notRunning
        }

        if let cb {
            var event = lattice_audio_event()
            event.kind = LATTICE_AUDIO_EVENT_STOPPED
            event.captured_at_ns = monotonicNanos()
            cb(&event, ctx)
        }
    }

    func markDestroyed() {
        stateLock.lock()
        streaming = false
        armed = false
        callback = nil
        context = nil
        stateLock.unlock()
        engine.inputNode.removeTap(onBus: 0)
        if engine.isRunning {
            engine.stop()
        }
    }

    private func ensurePermission() throws {
        switch AVCaptureDevice.authorizationStatus(for: .audio) {
        case .authorized:
            return
        case .notDetermined:
            let semaphore = DispatchSemaphore(value: 0)
            let granted = OSAllocatedUnfairLock(initialState: false)
            AVCaptureDevice.requestAccess(for: .audio) { ok in
                granted.withLock { $0 = ok }
                semaphore.signal()
            }
            semaphore.wait()
            if !granted.withLock({ $0 }) {
                throw BridgeFailure.permission("microphone permission denied")
            }
        case .denied, .restricted:
            throw BridgeFailure.permission("microphone permission denied")
        @unknown default:
            throw BridgeFailure.permission("microphone permission unknown")
        }
    }

    private func installTapIfNeeded() throws {
        let input = engine.inputNode
        let inputFormat = input.outputFormat(forBus: 0)
        guard inputFormat.sampleRate > 0, inputFormat.channelCount > 0 else {
            throw BridgeFailure.device("input device format is unavailable")
        }

        guard let canonical = AVAudioFormat(
            commonFormat: .pcmFormatFloat32,
            sampleRate: canonicalSampleRate,
            channels: 1,
            interleaved: true
        ) else {
            throw BridgeFailure.internalError("failed to create canonical AVAudioFormat")
        }

        // Use the standard input node (not AUVoiceIO) so AGC / noise suppression
        // / echo cancellation stay off by default.

        guard let converter = AVAudioConverter(from: inputFormat, to: canonical) else {
            throw BridgeFailure.device("AVAudioConverter setup failed")
        }

        self.canonicalFormat = canonical
        self.converter = converter

        input.removeTap(onBus: 0)
        let bufferSize: AVAudioFrameCount = 1_024
        input.installTap(onBus: 0, bufferSize: bufferSize, format: inputFormat) {
            [weak self] buffer, _ in
            self?.handleInputBuffer(buffer)
        }
    }

    private func startEngineIfNeeded() throws {
        if engine.isRunning {
            return
        }
        do {
            try engine.start()
        } catch {
            throw BridgeFailure.device("AVAudioEngine start failed: \(error)")
        }
    }

    private func handleInputBuffer(_ buffer: AVAudioPCMBuffer) {
        guard let converter, let canonicalFormat else { return }
        guard let converted = convert(buffer: buffer, converter: converter, format: canonicalFormat)
        else { return }
        guard let channelData = converted.floatChannelData?[0] else { return }

        let count = Int(converted.frameLength)
        let samples = UnsafeBufferPointer(start: channelData, count: count)
        let capturedAt = monotonicNanos()

        stateLock.lock()
        let isStreaming = streaming
        let isArmed = armed
        stateLock.unlock()

        if isStreaming {
            appendAndEmit(samples: samples, capturedAt: capturedAt)
        } else if isArmed {
            preRoll.push(samples)
        }
    }

    private func convert(
        buffer: AVAudioPCMBuffer,
        converter: AVAudioConverter,
        format: AVAudioFormat
    ) -> AVAudioPCMBuffer? {
        let ratio = format.sampleRate / buffer.format.sampleRate
        let capacity = max(
            AVAudioFrameCount(Double(buffer.frameLength) * ratio) + 32,
            1
        )
        guard let output = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: capacity) else {
            return nil
        }

        var error: NSError?
        let consumed = OSAllocatedUnfairLock(initialState: false)
        let inputBlock: AVAudioConverterInputBlock = { _, outStatus in
            let already = consumed.withLock { state -> Bool in
                if state {
                    return true
                }
                state = true
                return false
            }
            if already {
                outStatus.pointee = .noDataNow
                return nil
            }
            outStatus.pointee = .haveData
            return buffer
        }
        converter.convert(to: output, error: &error, withInputFrom: inputBlock)
        if error != nil {
            return nil
        }
        return output
    }

    private func appendAndEmit(samples: UnsafeBufferPointer<Float>, capturedAt: UInt64) {
        stateLock.lock()
        emitScratch.append(contentsOf: samples)
        var frames = [[Float]]()
        while emitScratch.count >= samplesPerEmitFrame {
            let frame = Array(emitScratch.prefix(samplesPerEmitFrame))
            emitScratch.removeFirst(samplesPerEmitFrame)
            frames.append(frame)
        }
        stateLock.unlock()

        for frame in frames {
            emitPcmFrame(frame, capturedAt: capturedAt)
        }
    }

    private func emitStarted(capturedAt: UInt64) {
        stateLock.lock()
        let cb = callback
        let ctx = context
        stateLock.unlock()
        guard let cb else { return }
        var event = lattice_audio_event()
        event.kind = LATTICE_AUDIO_EVENT_STARTED
        event.captured_at_ns = capturedAt
        cb(&event, ctx)
    }

    private func emitPcmFrame(_ samples: [Float], capturedAt: UInt64) {
        stateLock.lock()
        let cb = callback
        let ctx = context
        let sequence = nextSequence
        nextSequence &+= 1
        let diagnostics = enableDiagnostics
        stateLock.unlock()
        guard let cb else { return }

        var peak: Float = .nan
        var rms: Float = .nan
        var clipped: UInt8 = 0
        if diagnostics {
            var peakAbs: Float = 0
            var sumSq: Double = 0
            for sample in samples {
                let abs = sample.magnitude
                if abs > peakAbs { peakAbs = abs }
                if abs >= 0.999 { clipped = 1 }
                sumSq += Double(sample) * Double(sample)
            }
            peak = peakAbs
            rms = samples.isEmpty ? 0 : Float(sqrt(sumSq / Double(samples.count)))
        }

        samples.withUnsafeBufferPointer { ptr in
            var event = lattice_audio_event()
            event.kind = LATTICE_AUDIO_EVENT_FRAME
            event.captured_at_ns = capturedAt
            event.frame.sequence = sequence
            event.frame.captured_at_ns = capturedAt
            event.frame.frame_count = UInt32(samples.count)
            event.frame.samples = ptr.baseAddress
            event.frame.peak_abs = peak
            event.frame.rms = rms
            event.frame.clipped = clipped
            cb(&event, ctx)
        }
    }
}

func monotonicNanos() -> UInt64 {
    var info = mach_timebase_info_data_t()
    mach_timebase_info(&info)
    let t = mach_absolute_time()
    return t * UInt64(info.numer) / UInt64(info.denom)
}

enum CaptureRegistry {
    private struct State {
        var nextId: UInt64 = 1
        var captures: [UInt64: AudioCapture] = [:]
    }

    private static let state = OSAllocatedUnfairLock(initialState: State())

    static func allocateId() -> UInt64 {
        state.withLock { state in
            let id = state.nextId
            state.nextId += 1
            return id
        }
    }

    static func put(_ capture: AudioCapture) {
        state.withLock { state in
            state.captures[capture.id] = capture
        }
    }

    static func get(_ id: UInt64) -> AudioCapture? {
        state.withLock { state in
            state.captures[id]
        }
    }

    static func remove(_ id: UInt64) -> AudioCapture? {
        state.withLock { state in
            state.captures.removeValue(forKey: id)
        }
    }
}
