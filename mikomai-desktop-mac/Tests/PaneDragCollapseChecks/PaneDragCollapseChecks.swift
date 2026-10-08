import AppKit
import SwiftUI

@MainActor final class PaneState: ObservableObject {
    @Published var leftOpen = true
    @Published var rightOpen = true
    @Published var leftWidth: CGFloat = 248
    @Published var rightWidth: CGFloat = 330
}

struct PaneCheckWindow: View {
    @ObservedObject var state: PaneState
    var body: some View {
        HSplitView {
            if state.leftOpen {
                Color.gray.frame(minWidth: 180, idealWidth: state.leftWidth, maxWidth: 420)
                    .background(PaneInitialSizing(width: state.leftWidth, isHistoryPane: true))
                    .overlay(alignment: .trailing) {
                        PaneDragCollapse(isHistoryPane: true, onClose: { width in
                            state.leftWidth = width
                            state.leftOpen = false
                        }, onReopen: { width in
                            state.leftWidth = width
                            state.leftOpen = true
                        }, onResize: { state.leftWidth = $0 })
                            .frame(width: 8).offset(x: 4)
                    }
            }
            Color.white.frame(minWidth: 440, maxWidth: .infinity)
            if state.rightOpen {
                Color.gray.frame(minWidth: 180, idealWidth: state.rightWidth, maxWidth: 600)
                    .background(PaneInitialSizing(width: state.rightWidth, isHistoryPane: false))
                    .overlay(alignment: .leading) {
                        PaneDragCollapse(isHistoryPane: false, onClose: { width in
                            state.rightWidth = width
                            state.rightOpen = false
                        }, onReopen: { width in
                            state.rightWidth = width
                            state.rightOpen = true
                        }, onResize: { state.rightWidth = $0 })
                            .frame(width: 8).offset(x: -4)
                    }
            }
        }
    }
}

@main struct PaneDragChecks {
    @MainActor static func main() {
        let app = NSApplication.shared
        app.setActivationPolicy(.regular)
        app.finishLaunching()
        let state = PaneState()
        let host = NSHostingView(rootView: PaneCheckWindow(state: state))
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 1200, height: 600),
                              styleMask: [.titled], backing: .buffered, defer: false)
        window.contentView = host
        window.makeKeyAndOrderFront(nil)
        func pump(_ seconds: Double = 0.1) {
            RunLoop.main.run(until: Date().addingTimeInterval(seconds))
            host.layoutSubtreeIfNeeded()
        }
        func findSplit(_ view: NSView) -> NSSplitView? {
            if let split = view as? NSSplitView { return split }
            return view.subviews.lazy.compactMap { findSplit($0) }.first
        }
        pump()
        let split = findSplit(host)!
        @discardableResult func drag(divider: Int, translation: CGFloat, release: Bool = true) -> (CGFloat, Bool) -> Void {
            let x = split.arrangedSubviews[divider].frame.maxX + split.dividerThickness / 2
            let start = split.convert(NSPoint(x: x, y: 300), to: nil)
            func event(_ type: NSEvent.EventType, _ point: NSPoint) -> NSEvent {
                NSEvent.mouseEvent(with: type, location: point, modifierFlags: [],
                    timestamp: ProcessInfo.processInfo.systemUptime, windowNumber: window.windowNumber,
                    context: nil, eventNumber: 0, clickCount: 1, pressure: 1)!
            }
            func dispatch() {
                while let next = app.nextEvent(matching: [.leftMouseDown, .leftMouseDragged, .leftMouseUp],
                                              until: Date(), inMode: .default, dequeue: true) {
                    app.sendEvent(next)
                }
                pump(0.05)
            }
            func move(_ translation: CGFloat, _ release: Bool) {
                let end = NSPoint(x: start.x + translation, y: start.y)
                app.postEvent(event(.leftMouseDragged, end), atStart: false)
                if release { app.postEvent(event(.leftMouseUp, end), atStart: false) }
                dispatch()
            }
            app.postEvent(event(.leftMouseDown, start), atStart: false)
            move(translation, release)
            return move
        }
        split.setPosition(248, ofDividerAt: 0)
        pump()
        drag(divider: 0, translation: 30)
        precondition(state.leftOpen && state.rightOpen)
        precondition(abs(split.arrangedSubviews[0].frame.width - 278) < 2, "Manual resize must still work")
        let reverseLeft = drag(divider: 0, translation: -98, release: false)
        reverseLeft(-20, true)
        pump(0.35)
        precondition(state.leftOpen && split.arrangedSubviews[0].alphaValue == 1,
                     "Reversing during close must cancel the animation and its completion")
        precondition(abs(split.arrangedSubviews[0].frame.width - 258) < 2)
        let leftPane = split.arrangedSubviews[0]
        drag(divider: 0, translation: -78, release: false)
        precondition(state.leftOpen, "The pane must remain mounted during its closing animation")
        precondition(leftPane.alphaValue < 1, "The threshold must start a visible animation before mouse-up")
        pump(0.35)
        precondition(!state.leftOpen && state.rightOpen, "Dragging left must close only history")
        precondition(split.arrangedSubviews.count == 2)
        let rightWidth = split.arrangedSubviews.last!.frame.width
        let continueRight = drag(divider: 0, translation: rightWidth - 181, release: false)
        precondition(state.rightOpen, "Above the threshold must stay open")
        let rightPane = split.arrangedSubviews.last!
        continueRight(rightWidth - 180, false)
        precondition(rightPane.alphaValue < 1, "The right pane must animate without mouse-up")
        pump(0.35)
        precondition(!state.rightOpen, "The right pane must close automatically at the threshold")
        state.leftOpen = true
        state.rightOpen = true
        pump()
        precondition(split.arrangedSubviews.count == 3, "Both panes must reopen")
        precondition(abs(split.arrangedSubviews[0].frame.width - 258) < 2, "Reopen must restore history width before collapse")
        precondition(abs(split.arrangedSubviews[2].frame.width - rightWidth) < 2, "Reopen must restore right width before collapse")
        precondition(split.arrangedSubviews[0].alphaValue == 1 && split.arrangedSubviews[2].alphaValue == 1,
                     "Reopened panes must be fully visible")
        precondition(split.subviews.filter { $0 is PaneDragCollapse.ResizeView }.count == 2,
                     "Reopening must not leak old drag handles")
        drag(divider: 0, translation: 20)
        precondition(state.leftOpen && state.rightOpen, "Reopened panes must support resizing")
        drag(divider: 0, translation: -500, release: false)
        pump(0.35)
        precondition(!state.leftOpen && state.rightOpen, "A fast drag past the threshold must also close")
        state.leftOpen = true
        pump(0.35)
        precondition(abs(split.arrangedSubviews[0].frame.width - state.leftWidth) < 2)
        let rightStart = split.arrangedSubviews[2].frame.width
        let reverseRight = drag(divider: 1, translation: rightStart - 180, release: false)
        reverseRight(20, true)
        pump(0.35)
        precondition(state.rightOpen && split.arrangedSubviews[2].alphaValue == 1,
                     "Right-side reversal must also cancel stale close completion")
        drag(divider: 1, translation: split.arrangedSubviews[2].frame.width - 180, release: false)
        pump(0.35)
        precondition(!state.rightOpen && state.leftOpen)
        state.rightOpen = true
        pump(0.35)
        precondition(split.subviews.filter { $0 is PaneDragCollapse.ResizeView }.count == 2)
        let restoreLeft = drag(divider: 0, translation: -500, release: false)
        pump(0.35)
        precondition(!state.leftOpen)
        restoreLeft(15, false)
        pump(0.35)
        precondition(state.leftOpen, "A held drag must reopen history after collapse finishes")
        let expectedLeftWidth = split.arrangedSubviews[0].frame.width
        restoreLeft(35, false)
        pump(0.1)
        precondition(abs(split.arrangedSubviews[0].frame.width - expectedLeftWidth - 20) < 2,
                     "The original held drag must resize the reopened pane")
        restoreLeft(-500, false)
        pump(0.35)
        precondition(!state.leftOpen, "The same held drag must be able to close again")
        restoreLeft(25, true)
        pump(0.35)
        precondition(state.leftOpen && split.arrangedSubviews[0].alphaValue == 1)
        let restoreRight = drag(divider: 1, translation: 500, release: false)
        pump(0.35)
        precondition(!state.rightOpen)
        restoreRight(-20, false)
        pump(0.35)
        precondition(state.rightOpen, "A held drag must reopen the right pane after collapse finishes")
        let expectedRightWidth = split.arrangedSubviews[2].frame.width
        restoreRight(-40, true)
        pump(0.35)
        precondition(abs(split.arrangedSubviews[2].frame.width - expectedRightWidth - 20) < 2)
        drag(divider: 0, translation: -500, release: false)
        state.leftOpen = false
        pump(0.05)
        state.leftOpen = true
        pump(0.35)
        precondition(state.leftOpen && split.arrangedSubviews[0].alphaValue == 1,
                     "A completion from a detached pane must not close its replacement")
        precondition(split.subviews.filter { $0 is PaneDragCollapse.ResizeView }.count == 2)
        print("PASS: held-drag close/reopen/resize/reclose on both sides, animation reversal, saved widths and stale completion cancellation")
        window.orderOut(nil)
    }
}
