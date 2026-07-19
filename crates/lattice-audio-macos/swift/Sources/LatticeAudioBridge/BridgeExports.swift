import Foundation
import LatticeAudioBridgeC

/// C ABI exports for LatticeAudioBridge.
///
/// All entry points catch Swift errors and never unwind across the ABI.
/// Handles are opaque `UInt64` values (capture registry).

public let LATTICE_AUDIO_BRIDGE_ABI_VERSION: UInt32 = 1

@_cdecl("lattice_audio_bridge_abi_version")
public func lattice_audio_bridge_abi_version() -> UInt32 {
    LATTICE_AUDIO_BRIDGE_ABI_VERSION
}

@_cdecl("lattice_audio_capture_create")
public func lattice_audio_capture_create(
    pre_roll_ms: UInt32,
    enable_diagnostics: UInt8,
    out_capture: UnsafeMutablePointer<UInt64>?
) -> Int32 {
    bridgeCatch {
        guard let out_capture else {
            throw BridgeFailure.invalidArgument("out_capture is null")
        }
        let id = CaptureRegistry.allocateId()
        let capture = AudioCapture(
            id: id,
            preRollMs: pre_roll_ms,
            enableDiagnostics: enable_diagnostics != 0
        )
        CaptureRegistry.put(capture)
        out_capture.pointee = id
        return BridgeErrorCode.ok.rawValue
    }
}

@_cdecl("lattice_audio_capture_arm")
public func lattice_audio_capture_arm(capture: UInt64) -> Int32 {
    bridgeCatch {
        guard let audioCapture = CaptureRegistry.get(capture) else {
            throw BridgeFailure.invalidArgument("Unknown capture handle")
        }
        try audioCapture.arm()
        return BridgeErrorCode.ok.rawValue
    }
}

@_cdecl("lattice_audio_capture_start")
public func lattice_audio_capture_start(
    capture: UInt64,
    callback: lattice_audio_event_callback?,
    context: UnsafeMutableRawPointer?
) -> Int32 {
    bridgeCatch {
        guard let audioCapture = CaptureRegistry.get(capture) else {
            throw BridgeFailure.invalidArgument("Unknown capture handle")
        }
        try audioCapture.start(callback: callback, context: context)
        return BridgeErrorCode.ok.rawValue
    }
}

@_cdecl("lattice_audio_capture_stop")
public func lattice_audio_capture_stop(capture: UInt64) -> Int32 {
    bridgeCatch {
        guard let audioCapture = CaptureRegistry.get(capture) else {
            throw BridgeFailure.invalidArgument("Unknown capture handle")
        }
        try audioCapture.stop()
        return BridgeErrorCode.ok.rawValue
    }
}

@_cdecl("lattice_audio_capture_destroy")
public func lattice_audio_capture_destroy(capture: UInt64) {
    if let audioCapture = CaptureRegistry.remove(capture) {
        audioCapture.markDestroyed()
    }
}
