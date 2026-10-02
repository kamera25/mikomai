import SwiftUI
import AppKit

/// A read-only editor keeps its viewport when text changes; new lines follow
/// only when the reader was already at the end.
struct CoreDebugTerminal: NSViewRepresentable {
    let text: String
    let followsOutput: Bool

    func makeNSView(context: Context) -> NSScrollView { Self.makeEditor() }

    static func makeEditor() -> NSScrollView {
        let scroll = NSScrollView()
        scroll.hasVerticalScroller = true
        scroll.hasHorizontalScroller = true
        scroll.autohidesScrollers = true
        scroll.backgroundColor = NSColor(red: 0.06, green: 0.07, blue: 0.09, alpha: 1)
        let editor = NSTextView(frame: .zero)
        editor.isEditable = false
        editor.isSelectable = true
        editor.isRichText = false
        editor.font = .monospacedSystemFont(ofSize: 11, weight: .regular)
        editor.textColor = NSColor(red: 0.64, green: 0.9, blue: 0.72, alpha: 1)
        editor.backgroundColor = scroll.backgroundColor
        editor.textContainerInset = NSSize(width: 10, height: 10)
        editor.minSize = .zero
        editor.maxSize = NSSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)
        editor.isHorizontallyResizable = true
        editor.isVerticallyResizable = true
        editor.textContainer?.widthTracksTextView = false
        editor.textContainer?.containerSize = editor.maxSize
        editor.setAccessibilityLabel("デバッグログ")
        scroll.documentView = editor
        return scroll
    }

    func updateNSView(_ scroll: NSScrollView, context: Context) {
        Self.update(scroll, text: text, followsOutput: followsOutput)
    }

    static func update(_ scroll: NSScrollView, text: String, followsOutput: Bool) {
        guard let editor = scroll.documentView as? NSTextView, editor.string != text,
              let storage = editor.textStorage, let layout = editor.layoutManager,
              let container = editor.textContainer else { return }
        let origin = scroll.contentView.bounds.origin
        let visibleHeight = scroll.contentView.bounds.height
        let wasAtBottom = origin.y + visibleHeight >= editor.frame.height - 24
        let old = editor.string
        let selection = editor.selectedRanges
        let attributes: [NSAttributedString.Key: Any] = [
            .font: NSFont.monospacedSystemFont(ofSize: 11, weight: .regular),
            .foregroundColor: NSColor(red: 0.64, green: 0.9, blue: 0.72, alpha: 1)
        ]
        if !old.isEmpty, text.hasPrefix(old) {
            storage.append(NSAttributedString(string: String(text.dropFirst(old.count)), attributes: attributes))
        } else {
            storage.setAttributedString(NSAttributedString(string: text, attributes: attributes))
        }
        // Lay out the entire document before clamping the viewport to its new size.
        layout.ensureLayout(for: container)
        let used = layout.usedRect(for: container)
        editor.setFrameSize(NSSize(
            width: max(scroll.contentView.bounds.width, ceil(used.maxX) + 20),
            height: max(visibleHeight, ceil(used.maxY) + 20)
        ))
        let length = storage.length
        editor.selectedRanges = selection.map { value in
            let range = value.rangeValue
            let location = min(range.location, length)
            return NSValue(range: NSRange(location: location, length: min(range.length, length - location)))
        }
        let maximumY = max(0, editor.frame.height - visibleHeight)
        let y = followsOutput && wasAtBottom ? maximumY : min(origin.y, maximumY)
        // Always show the beginning of the line, never center wide JSON records.
        scroll.contentView.scroll(to: NSPoint(x: 0, y: y))
        scroll.reflectScrolledClipView(scroll.contentView)
    }
}
