import AppKit
import CoreGraphics
import Foundation

/// Screen Recording permission helpers (CoreGraphics TCC surface).
enum ScreenCapturePermission {
    private static let requestedKey = "lattice.capture.screenRecordingRequested"

    enum State: UInt32 {
        case unsupported = 0
        case notDetermined = 1
        case authorized = 2
        case denied = 3
        case restricted = 4
    }

    static func currentState() -> State {
        if CGPreflightScreenCaptureAccess() {
            return .authorized
        }
        if UserDefaults.standard.bool(forKey: requestedKey) {
            return .denied
        }
        return .notDetermined
    }

    @discardableResult
    static func requestAccess() -> State {
        UserDefaults.standard.set(true, forKey: requestedKey)
        if CGRequestScreenCaptureAccess() {
            return .authorized
        }
        return currentState()
    }

    static func openSystemSettings() throws {
        let urlString: String
        if #available(macOS 13.0, *) {
            urlString =
                "x-apple.systempreferences:com.apple.settings.PrivacySecurity.extension?Privacy_ScreenCapture"
        } else {
            urlString =
                "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture"
        }
        guard let url = URL(string: urlString) else {
            throw BridgeFailure.internalError("Invalid Screen Recording settings URL")
        }
        let opened = NSWorkspace.shared.open(url)
        if !opened {
            throw BridgeFailure.internalError("Failed to open Screen Recording settings")
        }
    }
}
