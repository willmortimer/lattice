import Foundation

/// Stable C ABI error codes for LatticeCaptureBridge.
/// Values must match `include/lattice_capture_bridge.h`.
enum BridgeErrorCode: Int32, Sendable {
    case ok = 0
    case invalidArg = -1
    case cancelled = -2
    case permission = -3
    case notFound = -4
    case internalError = -5
    case unsupported = -6
    case notImplemented = -7
}

enum BridgeFailure: Error, CustomStringConvertible, Sendable {
    case invalidArgument(String)
    case cancelled
    case permission(String)
    case notFound(String)
    case unsupported(String)
    case notImplemented(String)
    case internalError(String)

    var code: BridgeErrorCode {
        switch self {
        case .invalidArgument: return .invalidArg
        case .cancelled: return .cancelled
        case .permission: return .permission
        case .notFound: return .notFound
        case .unsupported: return .unsupported
        case .notImplemented: return .notImplemented
        case .internalError: return .internalError
        }
    }

    var description: String {
        switch self {
        case .invalidArgument(let message): return message
        case .cancelled: return "Capture cancelled"
        case .permission(let message): return message
        case .notFound(let message): return message
        case .unsupported(let message): return message
        case .notImplemented(let message): return message
        case .internalError(let message): return message
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
