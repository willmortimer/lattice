import AVFoundation
import FluidAudio
import Foundation
import LatticeVoiceBridgeC
import os

/// One utterance stream against a prepared Unified engine.
///
/// Streaming partials fire during `pushAudio`; `finishUtterance` runs
/// `StreamingUnifiedAsrManager.finish()` and emits one authoritative final.
final class VoiceSession: @unchecked Sendable {
    let id: UInt64
    private let engine: VoiceEngine
    private let callback: lattice_voice_event_callback
    private let context: UnsafeMutableRawPointer?
    private let lock = OSAllocatedUnfairLock()
    private var cancelled = false
    private var destroyed = false
    private var finishing = false
    private var revision: UInt64 = 0
    private var lastPartial: String = ""

    init(
        id: UInt64,
        engine: VoiceEngine,
        callback: lattice_voice_event_callback,
        context: UnsafeMutableRawPointer?
    ) {
        self.id = id
        self.engine = engine
        self.callback = callback
        self.context = context
    }

    func start() async throws {
        let manager = try engine.takeManager()
        try await manager.reset()
        await manager.setPartialTranscriptCallback { [weak self] text in
            self?.emitPartial(text)
        }
    }

    /// Copy samples and feed the Unified streamer. Never retains the caller's buffer.
    func pushAudio(samples: [Float]) async throws {
        try ensureActive()
        guard !samples.isEmpty else { return }

        let manager = try engine.takeManager()
        let buffer = try Self.makePCMBuffer(samples: samples, sampleRate: 16_000)
        try await manager.appendAudio(buffer)
        try await manager.processBufferedAudio()
        try ensureActive()
    }

    func finishUtterance() async throws {
        try lock.withLock { () throws in
            if cancelled { throw BridgeFailure.cancelled }
            if destroyed { throw BridgeFailure.session("Session was destroyed") }
            if finishing {
                throw BridgeFailure.session("finish_utterance already in progress")
            }
            finishing = true
        }

        do {
            let manager = try engine.takeManager()
            let text = try await manager.finish()
            try ensureActive()
            emitFinal(text)
            await manager.setPartialTranscriptCallback { _ in }
        } catch {
            lock.withLock { finishing = false }
            throw error
        }
    }

    func cancel() {
        let shouldDrop = lock.withLock { () -> Bool in
            if cancelled || destroyed { return false }
            cancelled = true
            return true
        }
        guard shouldDrop else { return }

        Task {
            if let manager = try? engine.takeManager() {
                await manager.setPartialTranscriptCallback { _ in }
                try? await manager.reset()
            }
        }
    }

    func markDestroyed() {
        cancel()
        lock.withLock { destroyed = true }
    }

    // MARK: - Events

    private func emitPartial(_ text: String) {
        let shouldEmit = lock.withLock { () -> Bool in
            if cancelled || destroyed || finishing { return false }
            revision += 1
            lastPartial = text
            return true
        }
        guard shouldEmit else { return }
        emitEvent(kind: LATTICE_VOICE_EVENT_PARTIAL, text: text, stablePrefixBytes: 0, errorCode: Int32(LATTICE_VOICE_OK))
    }

    private func emitFinal(_ text: String) {
        let shouldEmit = lock.withLock { () -> Bool in
            if cancelled || destroyed { return false }
            return true
        }
        guard shouldEmit else { return }
        emitEvent(
            kind: LATTICE_VOICE_EVENT_FINAL,
            text: text,
            stablePrefixBytes: UInt32(text.utf8.count),
            errorCode: Int32(LATTICE_VOICE_OK)
        )
    }

    func emitError(_ failure: BridgeFailure) {
        let shouldEmit = lock.withLock { () -> Bool in
            if cancelled || destroyed { return false }
            return true
        }
        guard shouldEmit else { return }
        emitEvent(
            kind: LATTICE_VOICE_EVENT_ERROR,
            text: failure.description,
            stablePrefixBytes: 0,
            errorCode: failure.code.rawValue
        )
    }

    private func emitEvent(
        kind: lattice_voice_event_kind_t,
        text: String,
        stablePrefixBytes: UInt32,
        errorCode: Int32
    ) {
        text.withCString { cString in
            var event = lattice_voice_event_t(
                kind: kind,
                text: cString,
                text_len: UInt32(text.utf8.count),
                stable_prefix_bytes: stablePrefixBytes,
                error_code: errorCode
            )
            callback(&event, context)
        }
    }

    private func ensureActive() throws {
        try lock.withLock {
            if cancelled { throw BridgeFailure.cancelled }
            if destroyed { throw BridgeFailure.session("Session was destroyed") }
        }
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
            throw BridgeFailure.internalError("Failed to create Float32 mono audio format")
        }
        guard
            let buffer = AVAudioPCMBuffer(
                pcmFormat: format,
                frameCapacity: AVAudioFrameCount(samples.count)
            )
        else {
            throw BridgeFailure.internalError("Failed to allocate AVAudioPCMBuffer")
        }
        buffer.frameLength = AVAudioFrameCount(samples.count)
        guard let channel = buffer.floatChannelData?[0] else {
            throw BridgeFailure.internalError("Missing float channel data")
        }
        samples.withUnsafeBufferPointer { src in
            guard let base = src.baseAddress else { return }
            channel.update(from: base, count: samples.count)
        }
        return buffer
    }
}

enum SessionRegistry {
    private struct State {
        var nextId: UInt64 = 1
        var sessions: [UInt64: VoiceSession] = [:]
    }

    private static let state = OSAllocatedUnfairLock(initialState: State())

    static func allocateId() -> UInt64 {
        state.withLock { state in
            let id = state.nextId
            state.nextId += 1
            return id
        }
    }

    static func put(_ session: VoiceSession) {
        state.withLock { state in
            state.sessions[session.id] = session
        }
    }

    static func get(_ id: UInt64) -> VoiceSession? {
        state.withLock { state in
            state.sessions[id]
        }
    }

    static func remove(_ id: UInt64) -> VoiceSession? {
        state.withLock { state in
            state.sessions.removeValue(forKey: id)
        }
    }
}
