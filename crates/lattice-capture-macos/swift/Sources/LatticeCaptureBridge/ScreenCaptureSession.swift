import AppKit
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

    static func enumerateDisplays() async throws -> [SCDisplay] {
        let content = try await SCShareableContent.excludingDesktopWindows(
            false,
            onScreenWindowsOnly: true
        )
        return content.displays
    }

    static func captureDisplay(displayId: CGDirectDisplayID) async throws -> CapturedPng {
        let content = try await SCShareableContent.excludingDesktopWindows(
            false,
            onScreenWindowsOnly: true
        )
        guard let display = content.displays.first(where: { $0.displayID == displayId }) else {
            throw BridgeFailure.notFound("Display \(displayId) not found")
        }
        return try await capture(filter: SCContentFilter(display: display, excludingWindows: []))
    }

    static func captureRegion(
        displayId: CGDirectDisplayID,
        region: CGRect
    ) async throws -> CapturedPng {
        let content = try await SCShareableContent.excludingDesktopWindows(
            false,
            onScreenWindowsOnly: true
        )
        guard let display = content.displays.first(where: { $0.displayID == displayId }) else {
            throw BridgeFailure.notFound("Display \(displayId) not found")
        }
        let filter = SCContentFilter(display: display, excludingWindows: [])
        var configuration = SCStreamConfiguration()
        configuration.sourceRect = region
        configuration.width = Int(region.width)
        configuration.height = Int(region.height)
        return try await capture(filter: filter, configuration: configuration)
    }

    private static func capture(
        filter: SCContentFilter,
        configuration: SCStreamConfiguration? = nil
    ) async throws -> CapturedPng {
        let image = try await SCScreenshotManager.captureImage(
            contentFilter: filter,
            configuration: configuration
        )
        guard let tiff = image.tiffRepresentation,
              let bitmap = NSBitmapImageRep(data: tiff),
              let png = bitmap.representation(using: .png, properties: [:])
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
