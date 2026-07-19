import Foundation

/// Stable C ABI error codes for LatticeVoiceBridge.
/// Values must match `include/lattice_voice_bridge.h`.
enum BridgeErrorCode: Int32, Sendable {
    case ok = 0
    case invalidArg = -1
    case notPrepared = -2
    case alreadyPrepared = -3
    case session = -4
    case cancelled = -5
    case internalError = -6
    case unsupported = -7
    case notFound = -8
}

/// Errors that stay inside the bridge; never thrown across the C ABI.
enum BridgeFailure: Error, CustomStringConvertible, Sendable {
    case invalidArgument(String)
    case notPrepared
    case alreadyPrepared
    case session(String)
    case cancelled
    case unsupported(String)
    case internalError(String)

    var code: BridgeErrorCode {
        switch self {
        case .invalidArgument: return .invalidArg
        case .notPrepared: return .notPrepared
        case .alreadyPrepared: return .alreadyPrepared
        case .session: return .session
        case .cancelled: return .cancelled
        case .unsupported: return .unsupported
        case .internalError: return .internalError
        }
    }

    var description: String {
        switch self {
        case .invalidArgument(let message):
            return message
        case .notPrepared:
            return "Engine is not prepared"
        case .alreadyPrepared:
            return "Engine is already prepared"
        case .session(let message):
            return message
        case .cancelled:
            return "Session was cancelled"
        case .unsupported(let message):
            return message
        case .internalError(let message):
            return message
        }
    }
}
