import AppKit
import Foundation

/// AppKit overlay scaffold for interactive region selection.
///
/// Full freeze + crosshair UI is deferred; this type documents the intended
/// integration point without calling `/usr/sbin/screencapture`.
enum RegionSelectorOverlay {
    static func captureInteractiveRegion() throws -> Never {
        throw BridgeFailure.notImplemented(
            "Interactive region overlay is not implemented yet; use fixed region capture"
        )
    }
}
