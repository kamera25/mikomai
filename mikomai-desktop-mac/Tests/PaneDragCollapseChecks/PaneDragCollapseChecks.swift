import AppKit
import SwiftUI

@MainActor final class PaneState: ObservableObject {
    @Published var leftOpen = true
    @Published var rightOpen = true
}

struct PaneCheckWindow: View {
    @ObservedObject var state: PaneState
    var body: some View {
        HSplitView {
            if state.leftOpen {
                Color.gray.frame(minWidth: 180, idealWidth: 248, maxWidth: 420)
                    .overlay(alignment: .trailing) {
                        PaneDragCollapse(isHistoryPane: true) { state.leftOpen = false }
                            .frame(width: 8).offset(x: 4)
                    }
            }
            Color.white.frame(minWidth: 440, maxWidth: .infinity)
            if state.rightOpen {
                Color.gray.frame(minWidth: 180, idealWidth: 330, maxWidth: 600)
                    .overlay(alignment: .leading) {
                        PaneDragCollapse(isHistoryPane: false) { state.rightOpen = false }
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
        func drag(divider: Int, translation: CGFloat, release: Bool = true) {
            let x = split.arrangedSubviews[divider].frame.maxX + split.dividerThickness / 2
            let start = split.convert(NSPoint(x: x, y: 300), to: nil)
            let end = NSPoint(x: start.x + translation, y: start.y)
            func event(_ type: NSEvent.EventType, _ point: NSPoint) -> NSEvent {
                NSEvent.mouseEvent(with: type, location: point, modifierFlags: [],
                    timestamp: ProcessInfo.processInfo.systemUptime, windowNumber: window.windowNumber,
                    context: nil, eventNumber: 0, clickCount: 1, pressure: 1)!
            }
            app.postEvent(event(.leftMouseDown, start), atStart: false)
            app.postEvent(event(.leftMouseDragged, end), atStart: false)
            if release { app.postEvent(event(.leftMouseUp, end), atStart: false) }
            while let next = app.nextEvent(matching: [.leftMouseDown, .leftMouseDragged, .leftMouseUp],
                                          until: Date(), inMode: .default, dequeue: true) {
                app.sendEvent(next)
            }
            pump(0.05)
        }
        split.setPosition(248, ofDividerAt: 0)
        pump()
        drag(divider: 0, translation: 30)
        precondition(state.leftOpen && state.rightOpen)
        precondition(abs(split.arrangedSubviews[0].frame.width - 278) < 2, "Manual resize must still work")
        let leftPane = split.arrangedSubviews[0]
        drag(divider: 0, translation: -98, release: false)
        precondition(state.leftOpen, "The pane must remain mounted during its closing animation")
        precondition(leftPane.alphaValue < 1, "The threshold must start a visible animation before mouse-up")
        pump(0.35)
        precondition(!state.leftOpen && state.rightOpen, "Dragging left must close only history")
        precondition(split.arrangedSubviews.count == 2)
        let rightWidth = split.arrangedSubviews.last!.frame.width
        drag(divider: 0, translation: rightWidth - 181)
        precondition(state.rightOpen, "Above the threshold must stay open")
        let rightPane = split.arrangedSubviews.last!
        drag(divider: 0, translation: 1, release: false)
        precondition(rightPane.alphaValue < 1, "The right pane must animate without mouse-up")
        pump(0.35)
        precondition(!state.rightOpen, "The right pane must close automatically at the threshold")
        state.leftOpen = true
        state.rightOpen = true
        pump()
        precondition(split.arrangedSubviews.count == 3, "Both panes must reopen")
        drag(divider: 0, translation: 20)
        precondition(state.leftOpen && state.rightOpen, "Reopened panes must support resizing")
        drag(divider: 0, translation: -500, release: false)
        pump(0.35)
        precondition(!state.leftOpen && state.rightOpen, "A fast drag past the threshold must also close")
        print("PASS: resizing, animation at the threshold before mouse-up, both sides, fast drag, and reopening")
        window.orderOut(nil)
    }
}
