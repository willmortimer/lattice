import AppKit
import CoreGraphics
import Foundation
import ScreenCaptureKit

/// ScreenCaptureKit-oriented capture helpers (no `/usr/sbin/screencapture`).
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
