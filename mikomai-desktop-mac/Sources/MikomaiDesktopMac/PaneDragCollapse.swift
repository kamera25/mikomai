import AppKit
import SwiftUI
import QuartzCore
import MikomaiDesktopCore

/// A wider divider hit target that tracks the pointer beyond the pane's minimum width.
struct PaneDragCollapse: NSViewRepresentable {
    let isHistoryPane: Bool
    let onClose: () -> Void

    final class ResizeView: NSView {
        var isHistoryPane = true
        var onClose: (() -> Void)?
        weak var paneView: NSView?
        private weak var splitView: NSSplitView?
        private var dragStart: (x: CGFloat, width: CGFloat, position: CGFloat, divider: Int)?
        private var isClosing = false

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
                    return
                }
                child = parent
            }
        }

        override func mouseDragged(with event: NSEvent) {
            guard !isClosing else { return }
            guard let start = dragStart, let split = splitView else { return }
            let translation = event.locationInWindow.x - start.x
            if PaneResizePolicy.shouldClose(startWidth: Double(start.width),
                                            translation: Double(translation), isHistoryPane: isHistoryPane) {
                closeWithAnimation()
                return
            }
            let proposedWidth = isHistoryPane ? start.width + translation : start.width - translation
            let width = PaneResizePolicy.clampedWidth(Double(proposedWidth),
                                                      maximumWidth: isHistoryPane ? 420 : 600)
            let adjustment = isHistoryPane ? CGFloat(width) - start.width : start.width - CGFloat(width)
            split.setPosition(start.position + adjustment, ofDividerAt: start.divider)
        }

        override func mouseUp(with event: NSEvent) {
            dragStart = nil
            splitView = nil
        }

        private func closeWithAnimation() {
            guard !isClosing, let pane = paneView else { return }
            isClosing = true
            // The split view enforces its minimum width. Animate the pane away
            // before removing it, rather than waiting for a mouse-up past that clamp.
            NSAnimationContext.runAnimationGroup { context in
                context.duration = 0.22
                context.timingFunction = CAMediaTimingFunction(name: .easeInEaseOut)
                pane.animator().alphaValue = 0
            } completionHandler: { [weak self] in
                DispatchQueue.main.async { self?.finishClosing() }
            }
        }

        private func finishClosing() {
            let pane = paneView
            onClose?()
            pane?.alphaValue = 1
            isClosing = false
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
                    monitor = NSEvent.addLocalMonitorForEvents(matching: [.leftMouseDown, .leftMouseDragged, .leftMouseUp]) { [weak self] event in
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
                    return
                }
                child = parent
            }
        }

        func detach() {
            if let monitor { NSEvent.removeMonitor(monitor) }
            monitor = nil
            NotificationCenter.default.removeObserver(self)
            handle.removeFromSuperview()
            handle.paneView = nil
            handle.onClose = nil
        }
    }

    func makeNSView(context: Context) -> ObserverView { ObserverView() }

    func updateNSView(_ view: ObserverView, context: Context) {
        view.handle.isHistoryPane = isHistoryPane
        view.handle.onClose = onClose
        view.schedulePosition()
    }

    static func dismantleNSView(_ view: ObserverView, coordinator: ()) {
        view.detach()
    }
}
