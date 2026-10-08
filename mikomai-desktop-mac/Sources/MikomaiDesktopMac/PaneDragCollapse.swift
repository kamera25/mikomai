import AppKit
import SwiftUI
import QuartzCore
import MikomaiDesktopCore

/// A wider divider hit target that tracks the pointer beyond the pane's minimum width.
struct PaneDragCollapse: NSViewRepresentable {
    let isHistoryPane: Bool
    let onClose: (CGFloat) -> Void
    var onReopen: (CGFloat) -> Void = { _ in }
    var onResize: (CGFloat) -> Void = { _ in }

    /// A drag belongs to the mouse press, not to the pane that may disappear.
    @MainActor final class DragSession {
        private static var active: [String: DragSession] = [:]
        let windowNumber: Int
        let isHistoryPane: Bool
        let startX: CGFloat
        let startWidth: CGFloat
        private let onReopen: (CGFloat) -> Void
        private var monitor: Any?
        private var windowCloseObserver: NSObjectProtocol?
        private weak var target: ResizeView?
        private var isClosed = false
        private var awaitingReopen = false
        private var lastDrag: NSEvent?
        private var key: String { "\(windowNumber):\(isHistoryPane)" }

        init(target: ResizeView, event: NSEvent, width: CGFloat, onReopen: @escaping (CGFloat) -> Void) {
            windowNumber = event.windowNumber
            isHistoryPane = target.isHistoryPane
            startX = event.locationInWindow.x
            startWidth = width
            self.onReopen = onReopen
            self.target = target
            for session in Array(Self.active.values) where session.windowNumber == windowNumber { session.stop() }
            Self.active[key] = self
            if let window = event.window {
                windowCloseObserver = NotificationCenter.default.addObserver(
                    forName: NSWindow.willCloseNotification, object: window, queue: .main
                ) { [weak self] _ in
                    DispatchQueue.main.async { self?.stop() }
                }
            }
            monitor = NSEvent.addLocalMonitorForEvents(matching: [.leftMouseDragged, .leftMouseUp]) { [weak self] event in
                guard let self, event.windowNumber == self.windowNumber else { return event }
                if event.type == .leftMouseUp {
                    self.target?.mouseUp(with: event)
                    self.stop()
                } else {
                    self.lastDrag = event
                    let translation = event.locationInWindow.x - self.startX
                    let proposed = self.isHistoryPane ? self.startWidth + translation : self.startWidth - translation
                    if self.isClosed, proposed > CGFloat(PaneResizePolicy.minimumWidth) {
                        self.isClosed = false
                        self.awaitingReopen = true
                        self.onReopen(CGFloat(PaneResizePolicy.clampedWidth(Double(proposed),
                            maximumWidth: self.isHistoryPane ? 420 : 600)))
                    } else if !self.awaitingReopen {
                        self.target?.mouseDragged(with: event)
                    }
                }
                return nil
            }
        }

        func didClose() {
            isClosed = true
            target = nil
        }

        static func resume(on target: ResizeView) {
            guard let window = target.window,
                  let session = active["\(window.windowNumber):\(target.isHistoryPane)"],
                  session.awaitingReopen else { return }
            session.target = target
            session.awaitingReopen = false
            target.resume(session)
            if let event = session.lastDrag { target.mouseDragged(with: event) }
        }

        private func stop() {
            if let monitor { NSEvent.removeMonitor(monitor) }
            monitor = nil
            if let windowCloseObserver { NotificationCenter.default.removeObserver(windowCloseObserver) }
            windowCloseObserver = nil
            target?.dragSession = nil
            if Self.active[key] === self { Self.active.removeValue(forKey: key) }
            target = nil
        }
    }

    final class ResizeView: NSView {
        var isHistoryPane = true
        var onClose: ((CGFloat) -> Void)?
        var onResize: ((CGFloat) -> Void)?
        var onReopen: ((CGFloat) -> Void)?
        var dragSession: DragSession?
        weak var paneView: NSView?
        private weak var splitView: NSSplitView?
        private var dragStart: (x: CGFloat, width: CGFloat, position: CGFloat, divider: Int)?
        private var isClosing = false
        private var closingGeneration = 0

        override var mouseDownCanMoveWindow: Bool { false }

        override func resetCursorRects() {
            addCursorRect(bounds, cursor: .resizeLeftRight)
        }

        func observe(_ event: NSEvent) -> NSEvent? {
            guard event.window === window else { return event }
            switch event.type {
            case .leftMouseDown:
                guard let split = superview as? NSSplitView,
                      frame.contains(split.convert(event.locationInWindow, from: nil)) else { return event }
                mouseDown(with: event)
                return dragStart == nil ? event : nil
            case .leftMouseDragged:
                guard dragStart != nil else { return event }
                mouseDragged(with: event)
                return nil
            case .leftMouseUp:
                guard dragStart != nil else { return event }
                mouseUp(with: event)
                return nil
            default: return event
            }
        }

        override func mouseDown(with event: NSEvent) {
            guard !isClosing else { return }
            dragStart = nil
            guard let paneView else { return }
            var child = paneView
            while let parent = child.superview {
                if let split = parent as? NSSplitView, split.isVertical {
                    guard let index = split.arrangedSubviews.firstIndex(of: child) else { return }
                    let divider = isHistoryPane ? index : index - 1
                    guard divider >= 0, divider < split.arrangedSubviews.count - 1 else { return }
                    splitView = split
                    dragStart = (event.locationInWindow.x, child.frame.width,
                                 split.arrangedSubviews[divider].frame.maxX, divider)
                    dragSession = DragSession(target: self, event: event, width: child.frame.width,
                                              onReopen: onReopen ?? { _ in })
                    return
                }
                child = parent
            }
        }

        func resume(_ session: DragSession) {
            guard let pane = paneView, let split = pane.superview as? NSSplitView,
                  let index = split.arrangedSubviews.firstIndex(of: pane) else { return }
            let divider = isHistoryPane ? index : index - 1
            guard divider >= 0, divider < split.arrangedSubviews.count - 1 else { return }
            dragSession = session
            splitView = split
            dragStart = (session.startX, session.startWidth, split.arrangedSubviews[divider].frame.maxX, divider)
        }

        override func mouseDragged(with event: NSEvent) {
            guard let start = dragStart, let split = splitView else { return }
            let translation = event.locationInWindow.x - start.x
            let shouldClose = PaneResizePolicy.shouldClose(startWidth: Double(start.width),
                                                          translation: Double(translation), isHistoryPane: isHistoryPane)
            if isClosing {
                guard !shouldClose else { return }
                cancelClosing()
            }
            if shouldClose {
                closeWithAnimation(restoreWidth: start.width)
                return
            }
            let proposedWidth = isHistoryPane ? start.width + translation : start.width - translation
            let width = PaneResizePolicy.clampedWidth(Double(proposedWidth),
                                                      maximumWidth: isHistoryPane ? 420 : 600)
            guard let pane = paneView else { return }
            let adjustment = isHistoryPane ? CGFloat(width) - pane.frame.width : pane.frame.width - CGFloat(width)
            split.setPosition(split.arrangedSubviews[start.divider].frame.maxX + adjustment, ofDividerAt: start.divider)
        }

        override func mouseUp(with event: NSEvent) {
            if !isClosing, dragStart != nil, let pane = paneView { onResize?(pane.frame.width) }
            dragStart = nil
            splitView = nil
        }

        private func closeWithAnimation(restoreWidth: CGFloat) {
            guard !isClosing, let pane = paneView else { return }
            isClosing = true
            closingGeneration += 1
            let generation = closingGeneration
            // The split view enforces its minimum width. Animate the pane away
            // before removing it, rather than waiting for a mouse-up past that clamp.
            NSAnimationContext.runAnimationGroup { context in
                context.duration = 0.22
                context.timingFunction = CAMediaTimingFunction(name: .easeInEaseOut)
                pane.animator().alphaValue = 0
            } completionHandler: { [weak self] in
                DispatchQueue.main.async { self?.finishClosing(generation: generation, restoreWidth: restoreWidth) }
            }
        }

        private func finishClosing(generation: Int, restoreWidth: CGFloat) {
            guard isClosing, generation == closingGeneration else { return }
            let pane = paneView
            dragSession?.didClose()
            onClose?(restoreWidth)
            pane?.alphaValue = 1
            isClosing = false
            dragStart = nil
            splitView = nil
        }

        func cancelClosing() {
            closingGeneration += 1
            isClosing = false
            if let pane = paneView {
                NSAnimationContext.runAnimationGroup { context in
                    context.duration = 0
                    pane.animator().alphaValue = 1
                }
            }
        }

        func reset() {
            cancelClosing()
            dragStart = nil
            splitView = nil
        }
    }

    final class ObserverView: NSView {
        let handle = ResizeView()
        private var scheduled = false
        private var monitor: Any?

        override func hitTest(_ point: NSPoint) -> NSView? { nil }

        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            if window == nil { detach() } else {
                if monitor == nil {
                    // NSSplitView intercepts hits on its native divider before
                    // child hit testing. Route that boundary through our handle.
                    monitor = NSEvent.addLocalMonitorForEvents(matching: [.leftMouseDown]) { [weak self] event in
                        guard let self else { return event }
                        return self.handle.observe(event)
                    }
                }
                schedulePosition()
            }
        }

        override func layout() {
            super.layout()
            schedulePosition()
        }

        func schedulePosition() {
            guard !scheduled, window != nil else { return }
            scheduled = true
            DispatchQueue.main.async { [weak self] in
                guard let self else { return }
                self.scheduled = false
                self.positionHandle()
            }
        }

        @objc private func positionHandle() {
            guard window != nil else { return }
            var child: NSView = self
            while let parent = child.superview {
                if let split = parent as? NSSplitView, split.isVertical,
                   let index = split.arrangedSubviews.firstIndex(of: child) {
                    let divider = handle.isHistoryPane ? index : index - 1
                    guard divider >= 0, divider < split.arrangedSubviews.count - 1 else { return }
                    if handle.superview !== split {
                        NotificationCenter.default.removeObserver(self)
                        // SwiftUI adds its own divider views above the arranged panes.
                        // Put the wider handle above those so the whole boundary works.
                        split.addSubview(handle, positioned: .above, relativeTo: nil)
                        NotificationCenter.default.addObserver(self, selector: #selector(positionHandle),
                            name: NSSplitView.didResizeSubviewsNotification, object: split)
                    }
                    handle.paneView = child
                    handle.frame = NSRect(x: split.arrangedSubviews[divider].frame.maxX - 4,
                                          y: split.bounds.minY, width: split.dividerThickness + 8,
                                          height: split.bounds.height)
                    split.window?.invalidateCursorRects(for: handle)
                    DragSession.resume(on: handle)
                    return
                }
                child = parent
            }
        }

        func detach() {
            handle.reset()
            if let monitor { NSEvent.removeMonitor(monitor) }
            monitor = nil
            NotificationCenter.default.removeObserver(self)
            handle.removeFromSuperview()
            handle.paneView = nil
        }
    }

    func makeNSView(context: Context) -> ObserverView { ObserverView() }

    func updateNSView(_ view: ObserverView, context: Context) {
        view.handle.isHistoryPane = isHistoryPane
        view.handle.onClose = onClose
        view.handle.onResize = onResize
        view.handle.onReopen = onReopen
        view.schedulePosition()
    }

    static func dismantleNSView(_ view: ObserverView, coordinator: ()) {
        view.detach()
        view.handle.onClose = nil
        view.handle.onResize = nil
        view.handle.onReopen = nil
    }
}

/// Apply a saved width once when a pane is mounted, including after reopening.
struct PaneInitialSizing: NSViewRepresentable {
    let width: CGFloat
    let isHistoryPane: Bool

    final class SizingView: NSView {
        var width: CGFloat = 248
        var isHistoryPane = true
        private var didSetWidth = false
        private var scheduled = false
        override func viewDidMoveToWindow() { super.viewDidMoveToWindow(); schedule() }
        override func layout() { super.layout(); schedule() }
        func schedule() {
            guard window != nil, !didSetWidth, !scheduled else { return }
            scheduled = true
            DispatchQueue.main.async { [weak self] in
                guard let self else { return }
                self.scheduled = false
                var child: NSView = self
                while let parent = child.superview {
                    if let split = parent as? NSSplitView, split.isVertical,
                       let index = split.arrangedSubviews.firstIndex(of: child), split.bounds.width > 0 {
                        let divider = self.isHistoryPane ? index : index - 1
                        guard divider >= 0, divider < split.arrangedSubviews.count - 1 else { return }
                        self.didSetWidth = true
                        let position = self.isHistoryPane ? child.frame.minX + self.width
                            : child.frame.maxX - self.width - split.dividerThickness
                        split.setPosition(position, ofDividerAt: divider)
                        return
                    }
                    child = parent
                }
            }
        }
    }
    func makeNSView(context: Context) -> SizingView { SizingView() }
    func updateNSView(_ view: SizingView, context: Context) {
        view.width = width
        view.isHistoryPane = isHistoryPane
        view.schedule()
    }
}
