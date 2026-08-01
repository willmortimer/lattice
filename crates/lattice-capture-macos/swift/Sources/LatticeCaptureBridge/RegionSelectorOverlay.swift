import AppKit
import CoreGraphics
import Foundation

/// Result of an interactive region selection (display-local, top-left origin).
///
/// Coordinates match `SCStreamConfiguration.sourceRect` / fixed region capture:
/// points relative to the top-left of the chosen display.
struct SelectedRegion: Sendable {
    let displayID: UInt32
    let rect: CGRect
}

/// AppKit per-display overlay for interactive region selection.
///
/// Interaction only: no ScreenCaptureKit, encode, or ingest. Escape cancels;
/// mouse-up on a non-empty drag confirms.
enum RegionSelectorOverlay {
    private static let minimumSelectionSide: CGFloat = 2

    /// Present overlays and block until the user confirms a region or cancels.
    static func selectRegion() throws -> SelectedRegion {
        if Thread.isMainThread {
            return try MainActor.assumeIsolated {
                try selectRegionOnMain()
            }
        }
        var result: Result<SelectedRegion, Error>?
        DispatchQueue.main.sync {
            result = Result { try selectRegionOnMain() }
        }
        guard let result else {
            throw BridgeFailure.internalError("Region selection result missing")
        }
        return try result.get()
    }

    @MainActor
    private static func selectRegionOnMain() throws -> SelectedRegion {
        let session = SelectionSession(minimumSelectionSide: minimumSelectionSide)
        session.begin()
        defer { session.tearDown() }

        // Nested run loop so a blocking C ABI caller can wait without freezing
        // AppKit event delivery (including Escape).
        while !session.isFinished {
            RunLoop.current.run(mode: .default, before: Date(timeIntervalSinceNow: 0.05))
        }
        return try session.takeResult()
    }
}

// MARK: - Session

@MainActor
private final class SelectionSession: NSObject {
    private let minimumSelectionSide: CGFloat
    private var overlays: [OverlayWindow] = []
    private var localKeyMonitor: Any?
    private var globalKeyMonitor: Any?
    private var dragStartGlobal: NSPoint?
    private var activeScreen: NSScreen?
    private var outcome: Result<SelectedRegion, Error>?

    private(set) var isFinished = false

    init(minimumSelectionSide: CGFloat) {
        self.minimumSelectionSide = minimumSelectionSide
    }

    func begin() {
        NSApp.activate(ignoringOtherApps: true)

        for screen in NSScreen.screens {
            let overlay = OverlayWindow(screen: screen)
            overlay.selectionDelegate = self
            overlays.append(overlay)
            overlay.orderFrontRegardless()
        }

        // Prefer the screen under the cursor as first key window.
        if let mouseScreen = screen(containing: NSEvent.mouseLocation),
           let match = overlays.first(where: { $0.targetScreen === mouseScreen })
        {
            match.makeKeyAndOrderFront(nil)
        } else {
            overlays.first?.makeKeyAndOrderFront(nil)
        }

        localKeyMonitor = NSEvent.addLocalMonitorForEvents(matching: [.keyDown]) { [weak self] event in
            if event.keyCode == 53 {
                self?.cancel()
                return nil
            }
            return event
        }
        globalKeyMonitor = NSEvent.addGlobalMonitorForEvents(matching: [.keyDown]) { [weak self] event in
            if event.keyCode == 53 {
                Task { @MainActor in
                    self?.cancel()
                }
            }
        }
    }

    func tearDown() {
        if let localKeyMonitor {
            NSEvent.removeMonitor(localKeyMonitor)
            self.localKeyMonitor = nil
        }
        if let globalKeyMonitor {
            NSEvent.removeMonitor(globalKeyMonitor)
            self.globalKeyMonitor = nil
        }
        for overlay in overlays {
            overlay.orderOut(nil)
            overlay.close()
        }
        overlays.removeAll()
    }

    func takeResult() throws -> SelectedRegion {
        guard let outcome else {
            throw BridgeFailure.internalError("Region selection finished without outcome")
        }
        return try outcome.get()
    }

    private func finish(_ result: Result<SelectedRegion, Error>) {
        guard !isFinished else { return }
        outcome = result
        isFinished = true
    }

    private func cancel() {
        finish(.failure(BridgeFailure.cancelled))
    }

    private func screen(containing point: NSPoint) -> NSScreen? {
        NSScreen.screens.first { NSMouseInRect(point, $0.frame, false) }
    }
}

@MainActor
extension SelectionSession: OverlaySelectionDelegate {
    fileprivate func overlayDidBeginDrag(_ overlay: OverlayWindow, atGlobal point: NSPoint) {
        dragStartGlobal = point
        activeScreen = overlay.targetScreen
        for window in overlays {
            window.clearSelection()
            window.isDimmedOnly = window !== overlay
        }
    }

    fileprivate func overlayDidDrag(_ overlay: OverlayWindow, toGlobal point: NSPoint) {
        guard let start = dragStartGlobal else { return }
        let rect = normalizedRect(from: start, to: point)
        overlay.updateSelection(globalRect: rect)
    }

    fileprivate func overlayDidEndDrag(_ overlay: OverlayWindow, atGlobal point: NSPoint) {
        guard let start = dragStartGlobal else {
            cancel()
            return
        }
        let screen = activeScreen ?? overlay.targetScreen
        dragStartGlobal = nil

        let globalRect = normalizedRect(from: start, to: point)
        guard globalRect.width >= minimumSelectionSide,
              globalRect.height >= minimumSelectionSide
        else {
            cancel()
            return
        }

        guard let selected = SelectedRegion.fromGlobalRect(globalRect, on: screen) else {
            finish(.failure(BridgeFailure.internalError("Unable to map selection to a display")))
            return
        }
        finish(.success(selected))
    }

    fileprivate func overlayDidCancel(_: OverlayWindow) {
        cancel()
    }
}

private func normalizedRect(from a: NSPoint, to b: NSPoint) -> NSRect {
    let x = min(a.x, b.x)
    let y = min(a.y, b.y)
    let w = abs(a.x - b.x)
    let h = abs(a.y - b.y)
    return NSRect(x: x, y: y, width: w, height: h)
}

extension SelectedRegion {
    /// Convert a Cocoa global rect (bottom-left origin) into display-local
    /// top-left coordinates for ScreenCaptureKit `sourceRect`.
    fileprivate static func fromGlobalRect(_ globalRect: NSRect, on screen: NSScreen) -> SelectedRegion? {
        guard let displayID = screenDisplayID(screen) else { return nil }
        let bounds = CGDisplayBounds(displayID)
        let clipped = globalRect.intersection(screen.frame)
        guard !clipped.isNull, clipped.width > 0, clipped.height > 0 else { return nil }

        let localX = clipped.origin.x - bounds.origin.x
        // Flip Y: Cocoa bottom-left → SCK top-left within the display.
        let localY = bounds.height - ((clipped.origin.y - bounds.origin.y) + clipped.height)
        let rect = CGRect(x: localX, y: localY, width: clipped.width, height: clipped.height)
        return SelectedRegion(displayID: UInt32(displayID), rect: rect)
    }
}

private func screenDisplayID(_ screen: NSScreen) -> CGDirectDisplayID? {
    let key = NSDeviceDescriptionKey("NSScreenNumber")
    guard let number = screen.deviceDescription[key] as? NSNumber else {
        return nil
    }
    return CGDirectDisplayID(number.uint32Value)
}

// MARK: - Overlay window / view

@MainActor
private protocol OverlaySelectionDelegate: AnyObject {
    func overlayDidBeginDrag(_ overlay: OverlayWindow, atGlobal point: NSPoint)
    func overlayDidDrag(_ overlay: OverlayWindow, toGlobal point: NSPoint)
    func overlayDidEndDrag(_ overlay: OverlayWindow, atGlobal point: NSPoint)
    func overlayDidCancel(_ overlay: OverlayWindow)
}

private final class OverlayWindow: NSWindow {
    weak var selectionDelegate: OverlaySelectionDelegate?
    let targetScreen: NSScreen
    private let selectionView: SelectionView

    var isDimmedOnly = false {
        didSet { selectionView.isDimmedOnly = isDimmedOnly }
    }

    init(screen: NSScreen) {
        self.targetScreen = screen
        selectionView = SelectionView(frame: NSRect(origin: .zero, size: screen.frame.size))
        super.init(
            contentRect: screen.frame,
            styleMask: .borderless,
            backing: .buffered,
            defer: false
        )
        setFrame(screen.frame, display: true)
        isOpaque = false
        backgroundColor = .clear
        hasShadow = false
        level = .screenSaver
        collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary]
        ignoresMouseEvents = false
        acceptsMouseMovedEvents = true
        contentView = selectionView
        selectionView.onEvent = { [weak self] event in
            self?.handleSelectionEvent(event)
        }
    }

    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { true }

    func clearSelection() {
        selectionView.selectionRect = nil
    }

    func updateSelection(globalRect: NSRect) {
        let bottomLeft = convertPoint(fromScreen: globalRect.origin)
        let topRight = convertPoint(
            fromScreen: NSPoint(x: globalRect.maxX, y: globalRect.maxY)
        )
        selectionView.selectionRect = NSRect(
            x: min(bottomLeft.x, topRight.x),
            y: min(bottomLeft.y, topRight.y),
            width: abs(topRight.x - bottomLeft.x),
            height: abs(topRight.y - bottomLeft.y)
        )
    }

    private func handleSelectionEvent(_ event: SelectionView.Event) {
        switch event {
        case .begin(let global):
            selectionDelegate?.overlayDidBeginDrag(self, atGlobal: global)
        case .drag(let global):
            selectionDelegate?.overlayDidDrag(self, toGlobal: global)
        case .end(let global):
            selectionDelegate?.overlayDidEndDrag(self, atGlobal: global)
        case .cancel:
            selectionDelegate?.overlayDidCancel(self)
        }
    }

    override func keyDown(with event: NSEvent) {
        if event.keyCode == 53 {
            selectionDelegate?.overlayDidCancel(self)
            return
        }
        super.keyDown(with: event)
    }
}

private final class SelectionView: NSView {
    enum Event {
        case begin(NSPoint)
        case drag(NSPoint)
        case end(NSPoint)
        case cancel
    }

    var onEvent: ((Event) -> Void)?
    var selectionRect: NSRect? {
        didSet { needsDisplay = true }
    }

    var isDimmedOnly = false {
        didSet { needsDisplay = true }
    }

    private var tracking = false

    override var acceptsFirstResponder: Bool { true }

    override func resetCursorRects() {
        addCursorRect(bounds, cursor: .crosshair)
    }

    override func draw(_ dirtyRect: NSRect) {
        let dim = NSColor.black.withAlphaComponent(0.45)
        dim.setFill()

        if let selectionRect, !isDimmedOnly {
            let path = NSBezierPath(rect: bounds)
            path.appendRect(selectionRect)
            path.windingRule = .evenOdd
            path.fill()

            NSColor.white.setStroke()
            let border = NSBezierPath(rect: selectionRect.insetBy(dx: 0.5, dy: 0.5))
            border.lineWidth = 1
            border.stroke()
        } else {
            NSBezierPath(rect: bounds).fill()
        }
    }

    override func mouseDown(with event: NSEvent) {
        tracking = true
        onEvent?(.begin(globalPoint(for: event)))
    }

    override func mouseDragged(with event: NSEvent) {
        guard tracking else { return }
        onEvent?(.drag(globalPoint(for: event)))
    }

    override func mouseUp(with event: NSEvent) {
        guard tracking else { return }
        tracking = false
        onEvent?(.end(globalPoint(for: event)))
    }

    override func rightMouseDown(with _: NSEvent) {
        tracking = false
        onEvent?(.cancel)
    }

    override func cancelOperation(_: Any?) {
        tracking = false
        onEvent?(.cancel)
    }

    private func globalPoint(for event: NSEvent) -> NSPoint {
        guard let window else { return event.locationInWindow }
        return window.convertPoint(toScreen: event.locationInWindow)
    }
}
