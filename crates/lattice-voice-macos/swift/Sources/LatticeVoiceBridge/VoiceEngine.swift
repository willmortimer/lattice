import FluidAudio
import Foundation
import os

/// Owns the loaded Unified streaming checkpoint (`parakeet-unified-320ms`).
/// Dual-path finals use `StreamingUnifiedAsrManager.finish()` from this manager;
/// the optional offline Unified encoder is intentionally not loaded for M1.
final class VoiceEngine: @unchecked Sendable {
    let id: UInt64
    let modelsRoot: URL
    private let lock = OSAllocatedUnfairLock()
    private var manager: StreamingUnifiedAsrManager?
    private var prepared = false
    private var destroyed = false

    init(id: UInt64, modelsRoot: URL) {
        self.id = id
        self.modelsRoot = modelsRoot
    }

    var isPrepared: Bool {
        lock.withLock { prepared && !destroyed }
    }

    func prepare() async throws {
        try lock.withLock { () throws in
            if destroyed {
                throw BridgeFailure.session("Engine was destroyed")
            }
            if prepared {
                throw BridgeFailure.alreadyPrepared
            }
        }

        let variant = StreamingModelVariant.parakeetUnified320ms
        guard let unifiedConfig = variant.unifiedConfig else {
            throw BridgeFailure.internalError("Missing UnifiedConfig for parakeet-unified-320ms")
        }

        let streaming = StreamingUnifiedAsrManager(
            config: unifiedConfig,
            encoderPrecision: .int8
        )

        try FileManager.default.createDirectory(
            at: modelsRoot,
            withIntermediateDirectories: true
        )
        try await streaming.loadModels(to: modelsRoot)

        try lock.withLock { () throws in
            if destroyed {
                throw BridgeFailure.session("Engine was destroyed during prepare")
            }
            manager = streaming
            prepared = true
        }
    }

    func takeManager() throws -> StreamingUnifiedAsrManager {
        try lock.withLock {
            if destroyed {
                throw BridgeFailure.session("Engine was destroyed")
            }
            guard prepared, let manager else {
                throw BridgeFailure.notPrepared
            }
            return manager
        }
    }

    func markDestroyed() {
        lock.withLock {
            destroyed = true
            manager = nil
            prepared = false
        }
    }
}

enum EngineRegistry {
    private struct State {
        var nextId: UInt64 = 1
        var engines: [UInt64: VoiceEngine] = [:]
    }

    private static let state = OSAllocatedUnfairLock(initialState: State())

    static func allocateId() -> UInt64 {
        state.withLock { state in
            let id = state.nextId
            state.nextId += 1
            return id
        }
    }

    static func put(_ engine: VoiceEngine) {
        state.withLock { state in
            state.engines[engine.id] = engine
        }
    }

    static func get(_ id: UInt64) -> VoiceEngine? {
        state.withLock { state in
            state.engines[id]
        }
    }

    static func remove(_ id: UInt64) -> VoiceEngine? {
        state.withLock { state in
            state.engines.removeValue(forKey: id)
        }
    }
}

enum ModelCacheResolver {
    /// Resolve Models directory: explicit path, else `LATTICE_VOICE_MODEL_CACHE`,
    /// else Application Support default.
    static func resolve(explicit: String?) throws -> URL {
        if let explicit, !explicit.isEmpty {
            return URL(fileURLWithPath: explicit, isDirectory: true)
        }
        if let env = ProcessInfo.processInfo.environment["LATTICE_VOICE_MODEL_CACHE"],
            !env.isEmpty
        {
            return URL(fileURLWithPath: env, isDirectory: true)
        }
        let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)
            .first
            ?? URL(fileURLWithPath: NSTemporaryDirectory(), isDirectory: true)
        return base
            .appendingPathComponent("Lattice", isDirectory: true)
            .appendingPathComponent("VoiceModels", isDirectory: true)
    }
}
