import AppKit
import Foundation

/// AppKit per-display overlay for click-to-target window selection.
///
/// Interaction only: no ScreenCaptureKit capture, encode, or ingest. Escape
/// cancels; click on a highlighted window confirms.
@available(macOS 14.0, *)
enum WindowSelectorOverlay {
    /// Present overlays and block until the user clicks a window or cancels.
    static func selectWindow() throws -> UInt64 {
        let windows = try runCaptureBlocking {
            try await ScreenCaptureSession.enumerateWindows()
        }
        guard !windows.isEmpty else {
            throw BridgeFailure.notFound("No capturable windows are on screen")
        }

        if Thread.isMainThread {
            return try MainActor.assumeIsolated {
                try selectWindowOnMain(windows: windows)
            }
        }
        var result: Result<UInt64, Error>?
        DispatchQueue.main.sync {
            result = Result { try selectWindowOnMain(windows: windows) }
        }
        guard let result else {
            throw BridgeFailure.internalError("Window selection result missing")
        }
        return try result.get()
    }

    @MainActor
    private static func selectWindowOnMain(
        windows: [ScreenCaptureSession.WindowInfo]
    ) throws -> UInt64 {
        let session = WindowSelectionSession(windows: windows)
        session.begin()
        defer { session.tearDown() }

        while !session.isFinished {
            RunLoop.current.run(mode: .default, before: Date(timeIntervalSinceNow: 0.05))
        }
        return try session.takeResult()
    }
}

// MARK: - Session

@available(macOS 14.0, *)
@MainActor
private final class WindowSelectionSession: NSObject {
    private let windows: [ScreenCaptureSession.WindowInfo]
    private var overlays: [WindowTargetOverlay] = []
    private var localKeyMonitor: Any?
    private var globalKeyMonitor: Any?
    private var localMouseMonitor: Any?
    private var globalMouseMonitor: Any?
    private var hovered: ScreenCaptureSession.WindowInfo?
    private var outcome: Result<UInt64, Error>?

    private(set) var isFinished = false

    init(windows: [ScreenCaptureSession.WindowInfo]) {
        self.windows = windows
    }

    func begin() {
        NSApp.activate(ignoringOtherApps: true)

        for screen in NSScreen.screens {
            let overlay = WindowTargetOverlay(screen: screen)
            overlay.selectionDelegate = self
            overlays.append(overlay)
            overlay.orderFrontRegardless()
        }

        if let mouseScreen = screen(containing: NSEvent.mouseLocation),
           let match = overlays.first(where: { $0.targetScreen === mouseScreen })
        {
            match.makeKeyAndOrderFront(nil)
        } else {
            overlays.first?.makeKeyAndOrderFront(nil)
        }

        updateHover(atGlobal: NSEvent.mouseLocation)

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

        // Only one overlay is key; track mouse globally so hover follows across displays.
        localMouseMonitor = NSEvent.addLocalMonitorForEvents(matching: [.mouseMoved]) { [weak self] event in
            self?.updateHover(atGlobal: NSEvent.mouseLocation)
            return event
        }
        globalMouseMonitor = NSEvent.addGlobalMonitorForEvents(matching: [.mouseMoved]) { [weak self] _ in
            Task { @MainActor in
                self?.updateHover(atGlobal: NSEvent.mouseLocation)
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
        if let localMouseMonitor {
            NSEvent.removeMonitor(localMouseMonitor)
            self.localMouseMonitor = nil
        }
        if let globalMouseMonitor {
            NSEvent.removeMonitor(globalMouseMonitor)
            self.globalMouseMonitor = nil
        }
        for overlay in overlays {
            overlay.orderOut(nil)
            overlay.close()
        }
        overlays.removeAll()
    }

    func takeResult() throws -> UInt64 {
        guard let outcome else {
            throw BridgeFailure.internalError("Window selection finished without outcome")
        }
        return try outcome.get()
    }

    private func finish(_ result: Result<UInt64, Error>) {
        guard !isFinished else { return }
        outcome = result
        isFinished = true
    }

    private func cancel() {
        finish(.failure(BridgeFailure.cancelled))
    }

    private func confirmHovered() {
        guard let hovered else { return }
        finish(.success(hovered.windowID))
    }

    private func window(containing point: NSPoint) -> ScreenCaptureSession.WindowInfo? {
        windows.first { NSMouseInRect(point, $0.cocoaFrame, false) }
    }

    private func updateHover(atGlobal point: NSPoint) {
        let match = window(containing: point)
        hovered = match
        for overlay in overlays {
            guard let match else {
                overlay.clearHighlight()
                continue
            }
            let clipped = match.cocoaFrame.intersection(overlay.targetScreen.frame)
            if clipped.isNull || clipped.width < 1 || clipped.height < 1 {
                overlay.clearHighlight()
            } else {
                overlay.updateHighlight(globalRect: clipped, title: match.title)
            }
        }
    }

    private func screen(containing point: NSPoint) -> NSScreen? {
        NSScreen.screens.first { NSMouseInRect(point, $0.frame, false) }
    }
}

@available(macOS 14.0, *)
@MainActor
extension WindowSelectionSession: WindowTargetDelegate {
    fileprivate func overlayDidMove(_: WindowTargetOverlay, toGlobal point: NSPoint) {
        updateHover(atGlobal: point)
    }

    fileprivate func overlayDidClick(_: WindowTargetOverlay, atGlobal point: NSPoint) {
        updateHover(atGlobal: point)
        confirmHovered()
    }

    fileprivate func overlayDidCancel(_: WindowTargetOverlay) {
        cancel()
    }
}

// MARK: - Overlay window / view

@available(macOS 14.0, *)
@MainActor
private protocol WindowTargetDelegate: AnyObject {
    func overlayDidMove(_ overlay: WindowTargetOverlay, toGlobal point: NSPoint)
    func overlayDidClick(_ overlay: WindowTargetOverlay, atGlobal point: NSPoint)
    func overlayDidCancel(_ overlay: WindowTargetOverlay)
}

@available(macOS 14.0, *)
private final class WindowTargetOverlay: NSWindow {
    weak var selectionDelegate: WindowTargetDelegate?
    let targetScreen: NSScreen
    private let highlightView: WindowHighlightView

    init(screen: NSScreen) {
        self.targetScreen = screen
        highlightView = WindowHighlightView(frame: NSRect(origin: .zero, size: screen.frame.size))
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
        contentView = highlightView
        highlightView.onEvent = { [weak self] event in
            self?.handleEvent(event)
        }
    }

    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { true }

    func clearHighlight() {
        highlightView.highlightRect = nil
        highlightView.title = nil
    }

    func updateHighlight(globalRect: NSRect, title: String) {
        let bottomLeft = convertPoint(fromScreen: globalRect.origin)
        let topRight = convertPoint(
            fromScreen: NSPoint(x: globalRect.maxX, y: globalRect.maxY)
        )
        highlightView.highlightRect = NSRect(
            x: min(bottomLeft.x, topRight.x),
            y: min(bottomLeft.y, topRight.y),
            width: abs(topRight.x - bottomLeft.x),
            height: abs(topRight.y - bottomLeft.y)
        )
        highlightView.title = title
    }

    private func handleEvent(_ event: WindowHighlightView.Event) {
        switch event {
        case .move(let global):
            selectionDelegate?.overlayDidMove(self, toGlobal: global)
        case .click(let global):
            selectionDelegate?.overlayDidClick(self, atGlobal: global)
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

@available(macOS 14.0, *)
private final class WindowHighlightView: NSView {
    enum Event {
        case move(NSPoint)
        case click(NSPoint)
        case cancel
    }

    var onEvent: ((Event) -> Void)?
    var highlightRect: NSRect? {
        didSet { needsDisplay = true }
    }

    var title: String? {
        didSet { needsDisplay = true }
    }

    override var acceptsFirstResponder: Bool { true }

    override func resetCursorRects() {
        addCursorRect(bounds, cursor: .pointingHand)
    }

    override func draw(_ dirtyRect: NSRect) {
        let dim = NSColor.black.withAlphaComponent(0.45)
        dim.setFill()

        if let highlightRect {
            let path = NSBezierPath(rect: bounds)
            path.appendRect(highlightRect)
            path.windingRule = .evenOdd
            path.fill()

            NSColor.systemBlue.setStroke()
            let border = NSBezierPath(rect: highlightRect.insetBy(dx: 0.5, dy: 0.5))
            border.lineWidth = 2
            border.stroke()

            if let title, !title.isEmpty {
                drawTitle(title, above: highlightRect)
            }
        } else {
            NSBezierPath(rect: bounds).fill()
        }
    }

    override func mouseMoved(with event: NSEvent) {
        onEvent?(.move(globalPoint(for: event)))
    }

    override func mouseDown(with event: NSEvent) {
        onEvent?(.click(globalPoint(for: event)))
    }

    override func rightMouseDown(with _: NSEvent) {
        onEvent?(.cancel)
    }

    override func cancelOperation(_: Any?) {
        onEvent?(.cancel)
    }

    private func globalPoint(for event: NSEvent) -> NSPoint {
        guard let window else { return event.locationInWindow }
        return window.convertPoint(toScreen: event.locationInWindow)
    }

    private func drawTitle(_ title: String, above rect: NSRect) {
        let attrs: [NSAttributedString.Key: Any] = [
            .font: NSFont.systemFont(ofSize: 13, weight: .medium),
            .foregroundColor: NSColor.white,
        ]
        let label = NSAttributedString(string: title, attributes: attrs)
        let size = label.size()
        let padding: CGFloat = 6
        var labelOrigin = NSPoint(
            x: rect.origin.x + 8,
            y: min(rect.maxY + 6, bounds.maxY - size.height - padding * 2)
        )
        if labelOrigin.y + size.height + padding * 2 > bounds.maxY {
            labelOrigin.y = max(rect.minY - size.height - 8, bounds.minY + padding)
        }
        let background = NSRect(
            x: labelOrigin.x - padding,
            y: labelOrigin.y - padding,
            width: size.width + padding * 2,
            height: size.height + padding * 2
        )
        NSColor.black.withAlphaComponent(0.75).setFill()
        NSBezierPath(roundedRect: background, xRadius: 4, yRadius: 4).fill()
        label.draw(at: labelOrigin)
    }
}
