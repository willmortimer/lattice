import Foundation

/// Stable C ABI error codes for LatticeAudioBridge.
/// Values must match `include/lattice_audio_bridge.h`.
enum BridgeErrorCode: Int32, Sendable {
    case ok = 0
    case invalidArg = -1
    case notArmed = -2
    case alreadyRunning = -3
    case permission = -4
    case device = -5
    case internalError = -6
    case unsupported = -7
    case notRunning = -8
}

enum BridgeFailure: Error, CustomStringConvertible, Sendable {
    case invalidArgument(String)
    case notArmed
    case alreadyRunning
    case permission(String)
    case device(String)
    case unsupported(String)
    case internalError(String)
    case notRunning

    var code: BridgeErrorCode {
        switch self {
        case .invalidArgument: return .invalidArg
        case .notArmed: return .notArmed
        case .alreadyRunning: return .alreadyRunning
        case .permission: return .permission
        case .device: return .device
        case .unsupported: return .unsupported
        case .internalError: return .internalError
        case .notRunning: return .notRunning
        }
    }

    var description: String {
        switch self {
        case .invalidArgument(let message): return message
        case .notArmed: return "Capture is not armed"
        case .alreadyRunning: return "Capture is already running"
        case .permission(let message): return message
        case .device(let message): return message
        case .unsupported(let message): return message
        case .internalError(let message): return message
        case .notRunning: return "Capture is not running"
        }
    }
}

@inline(__always)
func bridgeCatch(_ body: () throws -> Int32) -> Int32 {
    do {
        return try body()
    } catch let failure as BridgeFailure {
        return failure.code.rawValue
    } catch {
        return BridgeErrorCode.internalError.rawValue
    }
}
