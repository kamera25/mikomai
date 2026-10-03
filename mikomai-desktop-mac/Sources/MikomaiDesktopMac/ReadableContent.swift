import AppKit
import SwiftUI

/// A keyboard stop for displayed content, without turning it into an action or
/// replacing the original text selection, links or child controls.
private struct ReadableContentTarget: NSViewRepresentable {
    let title: String
    let text: String
    @Environment(\.isEnabled) private var isEnabled

    func makeNSView(context: Context) -> ReadableContentView { ReadableContentView() }
    func updateNSView(_ view: ReadableContentView, context: Context) {
        view.isEnabled = isEnabled
        view.setAccessibilityElement(true)
        view.setAccessibilityRole(.staticText)
        view.setAccessibilityLabel("\(title): \(text.isEmpty ? "未設定" : text)")
        view.setAccessibilityValue(text.isEmpty ? "未設定" : text)
        KeyboardNavigation.schedule(in: view.window)
    }
}

final class ReadableContentView: NSView {
    var isEnabled = true
    override var acceptsFirstResponder: Bool { isEnabled }
    override var canBecomeKeyView: Bool { isEnabled && !isHiddenOrHasHiddenAncestor }
    override func hitTest(_ point: NSPoint) -> NSView? { nil }
    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        KeyboardNavigation.schedule(in: window)
    }
    override func becomeFirstResponder() -> Bool {
        guard isEnabled else { return false }
        scrollToVisible(bounds)
        needsDisplay = true
        return true
    }
    override func resignFirstResponder() -> Bool { needsDisplay = true; return true }
    override func draw(_ dirtyRect: NSRect) {
        if window?.firstResponder === self {
            NSColor.keyboardFocusIndicatorColor.setStroke()
            let ring = NSBezierPath(roundedRect: bounds.insetBy(dx: 1, dy: 1), xRadius: 4, yRadius: 4)
            ring.lineWidth = 2
            ring.stroke()
        }
    }
}

extension View {
    /// Keep interactive descendants accessible when the displayed content
    /// includes Markdown links, code actions, or diagram controls.
    func keyboardReadable(_ title: String, text: String, preservesChildren: Bool = false) -> some View {
        accessibilityHidden(!preservesChildren)
            .overlay(ReadableContentTarget(title: title, text: text).allowsHitTesting(false))
    }
}
