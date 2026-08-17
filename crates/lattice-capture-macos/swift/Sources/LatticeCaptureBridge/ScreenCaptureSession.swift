import AppKit
import CoreGraphics
import Foundation
import ScreenCaptureKit

/// ScreenCaptureKit-oriented capture helpers (no `/usr/sbin/screencapture`).
///
/// PNG encoding here is FFI transport only (SCK → C ABI bytes). Clipboard,
/// WebP storage, and Capture Inbox ingest are owned by Rust command core.
@available(macOS 14.0, *)
enum ScreenCaptureSession {
    struct CapturedPng: Sendable {
        let width: UInt32
        let height: UInt32
        let pngData: Data
    }

    /// Sendable display metadata (SCDisplay itself is not Sendable).
    struct DisplayInfo: Sendable {
        let displayID: UInt32
        let width: UInt32
        let height: UInt32
    }

    /// Sendable window metadata (SCWindow itself is not Sendable).
    struct WindowInfo: Sendable {
        let windowID: UInt64
        let title: String
        let width: UInt32
        let height: UInt32
        /// Cocoa global coordinates (bottom-left origin) for click-to-target.
        let cocoaFrame: CGRect
    }

    static func enumerateDisplays() async throws -> [DisplayInfo] {
        let content = try await SCShareableContent.excludingDesktopWindows(
            false,
            onScreenWindowsOnly: true
        )
        return content.displays.map { display in
            DisplayInfo(
                displayID: UInt32(display.displayID),
                width: UInt32(display.width),
                height: UInt32(display.height)
            )
        }
    }

    static func enumerateWindows() async throws -> [WindowInfo] {
        let content = try await loadShareableContent()
        let excludedPids = Set(latticeExcludedApplications(from: content).map(\.processID))
        let cocoaFrames = cocoaFramesByWindowID()
        let zIndex = windowZOrderIndex()

        var rows: [WindowInfo] = []
        for window in content.windows {
            guard window.isOnScreen, window.windowLayer == 0 else { continue }
            if let app = window.owningApplication, excludedPids.contains(app.processID) {
                continue
            }
            let title = windowDisplayTitle(window)
            guard !title.isEmpty else { continue }
            let quartz = window.frame
            guard quartz.width >= 2, quartz.height >= 2 else { continue }
            let windowID = UInt64(window.windowID)
            let cocoaFrame = cocoaFrames[windowID] ?? quartzRectToCocoa(quartz)
            rows.append(
                WindowInfo(
                    windowID: windowID,
                    title: title,
                    width: UInt32(quartz.width.rounded(.towardZero)),
                    height: UInt32(quartz.height.rounded(.towardZero)),
                    cocoaFrame: cocoaFrame
                )
            )
        }

        rows.sort { lhs, rhs in
            let left = zIndex[lhs.windowID] ?? Int.max
            let right = zIndex[rhs.windowID] ?? Int.max
            return left < right
        }
        return rows
    }

    static func captureDisplay(displayId: UInt32) async throws -> CapturedPng {
        let content = try await loadShareableContent()
        guard let display = content.displays.first(where: { $0.displayID == CGDirectDisplayID(displayId) }) else {
            throw BridgeFailure.notFound("Display \(displayId) not found")
        }
        return try await capture(filter: contentFilter(display: display, content: content))
    }

    static func captureRegion(
        displayId: UInt32,
        region: CGRect
    ) async throws -> CapturedPng {
        let content = try await loadShareableContent()
        guard let display = content.displays.first(where: { $0.displayID == CGDirectDisplayID(displayId) }) else {
            throw BridgeFailure.notFound("Display \(displayId) not found")
        }
        let filter = contentFilter(display: display, content: content)
        let configuration = SCStreamConfiguration()
        configuration.sourceRect = region
        configuration.width = Int(region.width)
        configuration.height = Int(region.height)
        return try await capture(filter: filter, configuration: configuration)
    }

    static func captureWindow(windowId: UInt64) async throws -> CapturedPng {
        let content = try await loadShareableContent()
        guard let window = content.windows.first(where: { UInt64($0.windowID) == windowId }) else {
            throw BridgeFailure.notFound("Window \(windowId) not found")
        }
        let filter = SCContentFilter(desktopIndependentWindow: window)
        let configuration = SCStreamConfiguration()
        let scale = CGFloat(filter.pointPixelScale)
        let rect = filter.contentRect
        configuration.width = max(Int((rect.width * scale).rounded()), 1)
        configuration.height = max(Int((rect.height * scale).rounded()), 1)
        configuration.showsCursor = false
        return try await capture(filter: filter, configuration: configuration)
    }

    private static func loadShareableContent() async throws -> SCShareableContent {
        try await SCShareableContent.excludingDesktopWindows(
            false,
            onScreenWindowsOnly: true
        )
    }

    /// Exclude Lattice overlay/shelf windows from ScreenCaptureKit captures.
    private static func latticeExcludedApplications(
        from content: SCShareableContent
    ) -> [SCRunningApplication] {
        let bundleId = Bundle.main.bundleIdentifier
        let pid = ProcessInfo.processInfo.processIdentifier
        return content.applications.filter { app in
            if let bundleId, app.bundleIdentifier == bundleId {
                return true
            }
            return app.processID == pid
        }
    }

    private static func windowDisplayTitle(_ window: SCWindow) -> String {
        let title = window.title?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        if !title.isEmpty {
            return title
        }
        return window.owningApplication?.applicationName.trimmingCharacters(in: .whitespacesAndNewlines)
            ?? ""
    }

    /// Quartz window bounds (`kCGWindowBounds`) converted to Cocoa global rects.
    private static func cocoaFramesByWindowID() -> [UInt64: CGRect] {
        guard let list = CGWindowListCopyWindowInfo(
            [.optionOnScreenOnly, .excludeDesktopElements],
            kCGNullWindowID
        ) as? [[String: Any]] else {
            return [:]
        }
        var frames: [UInt64: CGRect] = [:]
        for entry in list {
            guard let number = entry[kCGWindowNumber as String] as? NSNumber else { continue }
            guard let bounds = entry[kCGWindowBounds as String] as? [String: Any],
                  let quartz = CGRect(dictionaryRepresentation: bounds as CFDictionary)
            else {
                continue
            }
            frames[number.uint64Value] = quartzRectToCocoa(quartz)
        }
        return frames
    }

    /// Front-to-back z-order from CoreGraphics (lower index is closer to front).
    private static func windowZOrderIndex() -> [UInt64: Int] {
        guard let list = CGWindowListCopyWindowInfo(
            [.optionOnScreenOnly, .excludeDesktopElements],
            kCGNullWindowID
        ) as? [[String: Any]] else {
            return [:]
        }
        var index: [UInt64: Int] = [:]
        for (position, entry) in list.enumerated() {
            guard let number = entry[kCGWindowNumber as String] as? NSNumber else { continue }
            index[number.uint64Value] = position
        }
        return index
    }

    private static func quartzRectToCocoa(_ quartz: CGRect) -> CGRect {
        let mainHeight = CGDisplayBounds(CGMainDisplayID()).height
        return CGRect(
            x: quartz.origin.x,
            y: mainHeight - quartz.origin.y - quartz.height,
            width: quartz.width,
            height: quartz.height
        )
    }

    private static func contentFilter(
        display: SCDisplay,
        content: SCShareableContent
    ) -> SCContentFilter {
        let excluding = latticeExcludedApplications(from: content)
        if excluding.isEmpty {
            return SCContentFilter(display: display, excludingWindows: [])
        }
        return SCContentFilter(
            display: display,
            excludingApplications: excluding,
            exceptingWindows: []
        )
    }

    private static func capture(
        filter: SCContentFilter,
        configuration: SCStreamConfiguration = SCStreamConfiguration()
    ) async throws -> CapturedPng {
        let cgImage = try await SCScreenshotManager.captureImage(
            contentFilter: filter,
            configuration: configuration
        )
        let bitmap = NSBitmapImageRep(cgImage: cgImage)
        guard let png = bitmap.representation(using: NSBitmapImageRep.FileType.png, properties: [:])
        else {
            throw BridgeFailure.internalError("Failed to encode screenshot as PNG")
        }
        return CapturedPng(
            width: UInt32(bitmap.pixelsWide),
            height: UInt32(bitmap.pixelsHigh),
            pngData: png
        )
    }
}

/// Run async ScreenCaptureKit work on a blocking C ABI thread.
@available(macOS 14.0, *)
func runCaptureBlocking<T: Sendable>(_ work: @escaping @Sendable () async throws -> T) throws -> T {
    let semaphore = DispatchSemaphore(value: 0)
    let box = ResultBox<T>()
    Task {
        do {
            let value = try await work()
            box.store(.success(value))
        } catch {
            box.store(.failure(error))
        }
        semaphore.signal()
    }
    semaphore.wait()
    return try box.take()
}

private final class ResultBox<T: Sendable>: @unchecked Sendable {
    private let lock = NSLock()
    private var result: Result<T, Error>?

    func store(_ result: Result<T, Error>) {
        lock.lock()
        self.result = result
        lock.unlock()
    }

    func take() throws -> T {
        lock.lock()
        defer { lock.unlock() }
        guard let result else {
            throw BridgeFailure.internalError("Capture result missing")
        }
        return try result.get()
    }
}
