import Foundation
import LatticeVoiceBridgeC

/// C ABI exports for LatticeVoiceBridge.
///
/// All entry points catch Swift errors and never unwind across the ABI.
/// Handles are opaque `UInt64` values (engine / session registries).

public let LATTICE_VOICE_BRIDGE_ABI_VERSION: UInt32 = 1

@_cdecl("lattice_voice_bridge_abi_version")
public func lattice_voice_bridge_abi_version() -> UInt32 {
    LATTICE_VOICE_BRIDGE_ABI_VERSION
}

@_cdecl("lattice_voice_engine_create")
public func lattice_voice_engine_create(
    model_cache_dir: UnsafePointer<CChar>?,
    out_engine: UnsafeMutablePointer<UInt64>?
) -> Int32 {
    bridgeCatch {
        guard let out_engine else {
            throw BridgeFailure.invalidArgument("out_engine is null")
        }
        let explicit: String? = model_cache_dir.map { String(cString: $0) }
        let modelsRoot = try ModelCacheResolver.resolve(explicit: explicit)
        let id = EngineRegistry.allocateId()
        let engine = VoiceEngine(id: id, modelsRoot: modelsRoot)
        EngineRegistry.put(engine)
        out_engine.pointee = id
        return BridgeErrorCode.ok.rawValue
    }
}

@_cdecl("lattice_voice_engine_prepare")
public func lattice_voice_engine_prepare(engine: UInt64) -> Int32 {
    bridgeCatch {
        guard let voiceEngine = EngineRegistry.get(engine) else {
            throw BridgeFailure.invalidArgument("Unknown engine handle")
        }
        try runBlocking {
            try await voiceEngine.prepare()
        }
        return BridgeErrorCode.ok.rawValue
    }
}

@_cdecl("lattice_voice_engine_destroy")
public func lattice_voice_engine_destroy(engine: UInt64) {
    if let voiceEngine = EngineRegistry.remove(engine) {
        voiceEngine.markDestroyed()
    }
}

@_cdecl("lattice_voice_session_start")
public func lattice_voice_session_start(
    engine: UInt64,
    callback: lattice_voice_event_callback?,
    context: UnsafeMutableRawPointer?,
    out_session: UnsafeMutablePointer<UInt64>?
) -> Int32 {
    bridgeCatch {
        guard let callback else {
            throw BridgeFailure.invalidArgument("callback is null")
        }
        guard let out_session else {
            throw BridgeFailure.invalidArgument("out_session is null")
        }
        guard let voiceEngine = EngineRegistry.get(engine) else {
            throw BridgeFailure.invalidArgument("Unknown engine handle")
        }
        guard voiceEngine.isPrepared else {
            throw BridgeFailure.notPrepared
        }

        let id = SessionRegistry.allocateId()
        let session = VoiceSession(
            id: id,
            engine: voiceEngine,
            callback: callback,
            context: context
        )
        SessionRegistry.put(session)

        do {
            try runBlocking {
                try await session.start()
            }
        } catch {
            _ = SessionRegistry.remove(id)
            throw error
        }

        out_session.pointee = id
        return BridgeErrorCode.ok.rawValue
    }
}

@_cdecl("lattice_voice_session_push_audio")
public func lattice_voice_session_push_audio(
    session: UInt64,
    samples: UnsafePointer<Float>?,
    sample_count: Int
) -> Int32 {
    bridgeCatch {
        guard sample_count >= 0 else {
            throw BridgeFailure.invalidArgument("sample_count must be non-negative")
        }
        if sample_count == 0 {
            return BridgeErrorCode.ok.rawValue
        }
        guard let samples else {
            throw BridgeFailure.invalidArgument("samples is null")
        }
        guard let voiceSession = SessionRegistry.get(session) else {
            throw BridgeFailure.invalidArgument("Unknown session handle")
        }

        // Copy immediately; never retain the caller's buffer beyond this call.
        let owned = Array(UnsafeBufferPointer(start: samples, count: sample_count))
        try runBlocking {
            try await voiceSession.pushAudio(samples: owned)
        }
        return BridgeErrorCode.ok.rawValue
    }
}

@_cdecl("lattice_voice_session_finish_utterance")
public func lattice_voice_session_finish_utterance(session: UInt64) -> Int32 {
    bridgeCatch {
        guard let voiceSession = SessionRegistry.get(session) else {
            throw BridgeFailure.invalidArgument("Unknown session handle")
        }
        do {
            try runBlocking {
                try await voiceSession.finishUtterance()
            }
            return BridgeErrorCode.ok.rawValue
        } catch let failure as BridgeFailure {
            voiceSession.emitError(failure)
            throw failure
        } catch {
            let wrapped = BridgeFailure.internalError(String(describing: error))
            voiceSession.emitError(wrapped)
            throw wrapped
        }
    }
}

@_cdecl("lattice_voice_session_cancel")
public func lattice_voice_session_cancel(session: UInt64) -> Int32 {
    bridgeCatch {
        guard let voiceSession = SessionRegistry.get(session) else {
            throw BridgeFailure.invalidArgument("Unknown session handle")
        }
        voiceSession.cancel()
        return BridgeErrorCode.ok.rawValue
    }
}

@_cdecl("lattice_voice_session_destroy")
public func lattice_voice_session_destroy(session: UInt64) {
    if let voiceSession = SessionRegistry.remove(session) {
        voiceSession.markDestroyed()
    }
}

// MARK: - ABI helpers

@inline(__always)
private func bridgeCatch(_ body: () throws -> Int32) -> Int32 {
    do {
        return try body()
    } catch let failure as BridgeFailure {
        return failure.code.rawValue
    } catch {
        return BridgeErrorCode.internalError.rawValue
    }
}

/// Run an async FluidAudio call on a dedicated cooperative pool and block the
/// C caller until it completes. Errors stay in-process.
private func runBlocking<T: Sendable>(_ operation: @escaping @Sendable () async throws -> T) throws
    -> T
{
    let box = ResultBox<T>()
    let semaphore = DispatchSemaphore(value: 0)
    Task.detached(priority: .userInitiated) {
        do {
            let value = try await operation()
            box.set(.success(value))
        } catch {
            box.set(.failure(error))
        }
        semaphore.signal()
    }
    semaphore.wait()
    return try box.get()
}

private final class ResultBox<T: Sendable>: @unchecked Sendable {
    private let lock = NSLock()
    private var result: Result<T, Error>?

    func set(_ result: Result<T, Error>) {
        lock.lock()
        self.result = result
        lock.unlock()
    }

    func get() throws -> T {
        lock.lock()
        defer { lock.unlock() }
        guard let result else {
            throw BridgeFailure.internalError("Missing async result")
        }
        return try result.get()
    }
}
