import CoreGraphics
import Foundation
import LatticeCaptureBridgeC

/// C ABI exports for LatticeCaptureBridge.
///
/// All entry points catch Swift errors and never unwind across the ABI.
public let LATTICE_CAPTURE_BRIDGE_ABI_VERSION: UInt32 = 1

@_cdecl("lattice_capture_bridge_abi_version")
public func lattice_capture_bridge_abi_version() -> UInt32 {
    LATTICE_CAPTURE_BRIDGE_ABI_VERSION
}

@_cdecl("lattice_capture_enumerate_displays")
public func lattice_capture_enumerate_displays(
    out: UnsafeMutablePointer<lattice_capture_display_info_t>?,
    outCapacity: UInt32,
    outCount: UnsafeMutablePointer<UInt32>?
) -> Int32 {
    bridgeCatch {
        guard let outCount else {
            throw BridgeFailure.invalidArgument("out_count is null")
        }
        if outCapacity > 0 && out == nil {
            throw BridgeFailure.invalidArgument("out is null")
        }

        if #available(macOS 14.0, *) {
            let displays = try runCaptureBlocking {
                try await ScreenCaptureSession.enumerateDisplays()
            }
            let limit = min(Int(outCapacity), displays.count)
            if limit > 0, let out {
                for index in 0 ..< limit {
                    let display = displays[index]
                    out[index] = lattice_capture_display_info_t(
                        display_id: UInt32(display.displayID),
                        width: UInt32(display.width),
                        height: UInt32(display.height)
                    )
                }
            }
            outCount.pointee = UInt32(limit)
            return BridgeErrorCode.ok.rawValue
        }

        throw BridgeFailure.unsupported("ScreenCaptureKit requires macOS 14+")
    }
}

@_cdecl("lattice_capture_enumerate_windows")
public func lattice_capture_enumerate_windows(
    out: UnsafeMutablePointer<lattice_capture_window_info_t>?,
    outCapacity: UInt32,
    outCount: UnsafeMutablePointer<UInt32>?
) -> Int32 {
    bridgeCatch {
        guard let outCount else {
            throw BridgeFailure.invalidArgument("out_count is null")
        }
        if outCapacity > 0 && out == nil {
            throw BridgeFailure.invalidArgument("out is null")
        }

        if #available(macOS 14.0, *) {
            let windows = try runCaptureBlocking {
                try await ScreenCaptureSession.enumerateWindows()
            }
            let limit = min(Int(outCapacity), windows.count)
            if limit > 0, let out {
                for index in 0 ..< limit {
                    var info = lattice_capture_window_info_t()
                    writeWindowInfo(windows[index], into: &info)
                    out[index] = info
                }
            }
            outCount.pointee = UInt32(limit)
            return BridgeErrorCode.ok.rawValue
        }

        throw BridgeFailure.unsupported("ScreenCaptureKit requires macOS 14+")
    }
}

@_cdecl("lattice_capture_capture_display")
public func lattice_capture_capture_display(
    displayId: UInt32,
    outImage: UnsafeMutablePointer<lattice_capture_image_out_t>?
) -> Int32 {
    bridgeCatch {
        guard let outImage else {
            throw BridgeFailure.invalidArgument("out_image is null")
        }
        if #available(macOS 14.0, *) {
            let captured = try runCaptureBlocking {
                try await ScreenCaptureSession.captureDisplay(displayId: displayId)
            }
            try writeImage(captured, into: outImage)
            return BridgeErrorCode.ok.rawValue
        }
        throw BridgeFailure.unsupported("ScreenCaptureKit requires macOS 14+")
    }
}

@_cdecl("lattice_capture_capture_window")
public func lattice_capture_capture_window(
    windowId: UInt64,
    outImage: UnsafeMutablePointer<lattice_capture_image_out_t>?
) -> Int32 {
    bridgeCatch {
        guard let outImage else {
            throw BridgeFailure.invalidArgument("out_image is null")
        }
        if #available(macOS 14.0, *) {
            let captured = try runCaptureBlocking {
                try await ScreenCaptureSession.captureWindow(windowId: windowId)
            }
            try writeImage(captured, into: outImage)
            return BridgeErrorCode.ok.rawValue
        }
        throw BridgeFailure.unsupported("ScreenCaptureKit requires macOS 14+")
    }
}

@_cdecl("lattice_capture_capture_region")
public func lattice_capture_capture_region(
    displayId: UInt32,
    region: UnsafePointer<lattice_capture_region_t>?,
    outImage: UnsafeMutablePointer<lattice_capture_image_out_t>?
) -> Int32 {
    bridgeCatch {
        guard let region, let outImage else {
            throw BridgeFailure.invalidArgument("region or out_image is null")
        }
        if #available(macOS 14.0, *) {
            let rect = CGRect(
                x: CGFloat(region.pointee.x),
                y: CGFloat(region.pointee.y),
                width: CGFloat(region.pointee.width),
                height: CGFloat(region.pointee.height)
            )
            let captured = try runCaptureBlocking {
                try await ScreenCaptureSession.captureRegion(
                    displayId: displayId,
                    region: rect
                )
            }
            try writeImage(captured, into: outImage)
            return BridgeErrorCode.ok.rawValue
        }
        throw BridgeFailure.unsupported("ScreenCaptureKit requires macOS 14+")
    }
}

@_cdecl("lattice_capture_select_interactive_region")
public func lattice_capture_select_interactive_region(
    outDisplayId: UnsafeMutablePointer<UInt32>?,
    outRegion: UnsafeMutablePointer<lattice_capture_region_t>?
) -> Int32 {
    bridgeCatch {
        guard let outDisplayId, let outRegion else {
            throw BridgeFailure.invalidArgument("out_display_id or out_region is null")
        }
        let selected = try RegionSelectorOverlay.selectRegion()
        let width = selected.rect.width.rounded(.towardZero)
        let height = selected.rect.height.rounded(.towardZero)
        guard width >= 1, height >= 1 else {
            throw BridgeFailure.cancelled
        }
        outDisplayId.pointee = selected.displayID
        outRegion.pointee = lattice_capture_region_t(
            x: Int32(selected.rect.origin.x.rounded(.towardZero)),
            y: Int32(selected.rect.origin.y.rounded(.towardZero)),
            width: UInt32(width),
            height: UInt32(height)
        )
        return BridgeErrorCode.ok.rawValue
    }
}

@_cdecl("lattice_capture_select_interactive_window")
public func lattice_capture_select_interactive_window(
    outWindowId: UnsafeMutablePointer<UInt64>?
) -> Int32 {
    bridgeCatch {
        guard let outWindowId else {
            throw BridgeFailure.invalidArgument("out_window_id is null")
        }
        if #available(macOS 14.0, *) {
            let windowId = try WindowSelectorOverlay.selectWindow()
            guard windowId > 0 else {
                throw BridgeFailure.cancelled
            }
            outWindowId.pointee = windowId
            return BridgeErrorCode.ok.rawValue
        }
        throw BridgeFailure.unsupported("ScreenCaptureKit requires macOS 14+")
    }
}

@_cdecl("lattice_capture_capture_interactive_region")
public func lattice_capture_capture_interactive_region(
    outImage: UnsafeMutablePointer<lattice_capture_image_out_t>?
) -> Int32 {
    bridgeCatch {
        guard let outImage else {
            throw BridgeFailure.invalidArgument("out_image is null")
        }
        // Selection is interaction-only; capture reuses the fixed-region SCK path.
        let selected = try RegionSelectorOverlay.selectRegion()
        if #available(macOS 14.0, *) {
            let captured = try runCaptureBlocking {
                try await ScreenCaptureSession.captureRegion(
                    displayId: selected.displayID,
                    region: selected.rect
                )
            }
            try writeImage(captured, into: outImage)
            return BridgeErrorCode.ok.rawValue
        }
        throw BridgeFailure.unsupported("ScreenCaptureKit requires macOS 14+")
    }
}

@_cdecl("lattice_capture_image_release")
public func lattice_capture_image_release(image: UnsafeMutablePointer<lattice_capture_image_out_t>?) {
    guard let image else { return }
    if image.pointee.png_bytes != nil {
        image.pointee.png_bytes.deallocate()
        image.pointee.png_bytes = nil
        image.pointee.png_len = 0
    }
}

@_cdecl("lattice_capture_permission_status")
public func lattice_capture_permission_status(
    outStatus: UnsafeMutablePointer<lattice_capture_permission_status_t>?
) -> Int32 {
    bridgeCatch {
        guard let outStatus else {
            throw BridgeFailure.invalidArgument("out_status is null")
        }
        outStatus.pointee = lattice_capture_permission_status_t(
            state: ScreenCapturePermission.currentState().rawValue
        )
        return BridgeErrorCode.ok.rawValue
    }
}

@_cdecl("lattice_capture_permission_request")
public func lattice_capture_permission_request(
    outStatus: UnsafeMutablePointer<lattice_capture_permission_status_t>?
) -> Int32 {
    bridgeCatch {
        guard let outStatus else {
            throw BridgeFailure.invalidArgument("out_status is null")
        }
        let state = ScreenCapturePermission.requestAccess()
        outStatus.pointee = lattice_capture_permission_status_t(state: state.rawValue)
        return BridgeErrorCode.ok.rawValue
    }
}

@_cdecl("lattice_capture_permission_open_settings")
public func lattice_capture_permission_open_settings() -> Int32 {
    bridgeCatch {
        try ScreenCapturePermission.openSystemSettings()
        return BridgeErrorCode.ok.rawValue
    }
}

@available(macOS 14.0, *)
private func writeImage(
    _ captured: ScreenCaptureSession.CapturedPng,
    into outImage: UnsafeMutablePointer<lattice_capture_image_out_t>
) throws {
    let count = captured.pngData.count
    guard count > 0 else {
        throw BridgeFailure.internalError("PNG payload is empty")
    }
    let buffer = UnsafeMutablePointer<UInt8>.allocate(capacity: count)
    captured.pngData.copyBytes(to: buffer, count: count)
    outImage.pointee = lattice_capture_image_out_t(
        width: captured.width,
        height: captured.height,
        png_bytes: buffer,
        png_len: UInt32(count)
    )
}

@available(macOS 14.0, *)
private func writeWindowInfo(
    _ window: ScreenCaptureSession.WindowInfo,
    into info: inout lattice_capture_window_info_t
) {
    info.window_id = window.windowID
    info.width = window.width
    info.height = window.height
    withUnsafeMutableBytes(of: &info.title) { buffer in
        buffer.initializeMemory(as: UInt8.self, repeating: 0)
        let bytes = window.title.utf8.prefix(max(buffer.count, 1) - 1)
        var offset = 0
        for byte in bytes {
            buffer.storeBytes(of: byte, toByteOffset: offset, as: UInt8.self)
            offset += 1
        }
    }
}
