import AppKit
import SwiftUI

/// Checks the composition state before AppKit consumes the Return event.
/// Checking afterwards would mistake the Return that commits Japanese text
/// for a request to send that text.
final class ChatComposerTextView: NSTextView {
    var onSubmit: () -> Void = {}
    var onEscape: () -> Void = {}

    override func keyDown(with event: NSEvent) {
        let modifiers = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
        if (event.keyCode == 36 || event.keyCode == 76), !hasMarkedText() {
            if modifiers.contains(.shift) {
                if isEditable { insertNewline(nil) }
                return
            }
            if modifiers.intersection([.option, .control]).isEmpty {
                if isEditable { onSubmit() }
                return
            }
        }
        if event.keyCode == 53, !hasMarkedText() {
            onEscape()
            return
        }
        super.keyDown(with: event)
    }

    override func draw(_ dirtyRect: NSRect) {
        super.draw(dirtyRect)
        if string.isEmpty, !hasMarkedText() {
            let placeholder = "質問を入力… (Enter で送信、Shift+Enter で改行)"
            placeholder.draw(at: NSPoint(x: textContainerInset.width, y: textContainerInset.height),
                             withAttributes: [.font: font ?? NSFont.systemFont(ofSize: 13),
                                              .foregroundColor: NSColor.placeholderTextColor])
        }
    }
}

private final class ChatComposerScrollView: NSScrollView {
    var focusOnAttachment = false

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        if focusOnAttachment, let editor = documentView, let window {
            window.makeFirstResponder(editor)
        }
    }
}

struct ChatComposer: NSViewRepresentable {
    @Binding var text: String
    @Binding var isFocused: Bool
    let isEnabled: Bool
    let onSubmit: () -> Void
    let onEscape: () -> Void

    func makeCoordinator() -> Coordinator { Coordinator(self) }

    func makeNSView(context: Context) -> NSScrollView {
        let scroll = ChatComposerScrollView()
        scroll.drawsBackground = false
        scroll.hasVerticalScroller = true
        scroll.autohidesScrollers = true
        scroll.borderType = .noBorder
        scroll.focusOnAttachment = isFocused

        let editor = ChatComposerTextView(frame: .zero)
        editor.isRichText = false
        editor.allowsUndo = true
        editor.drawsBackground = false
        editor.font = .systemFont(ofSize: 13)
        editor.textColor = .textColor
        editor.insertionPointColor = .textColor
        editor.textContainerInset = NSSize(width: 4, height: 4)
        editor.textContainer?.lineFragmentPadding = 0
        editor.isHorizontallyResizable = false
        editor.isVerticallyResizable = true
        editor.autoresizingMask = [.width]
        editor.maxSize = NSSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)
        editor.textContainer?.widthTracksTextView = true
        editor.isAutomaticQuoteSubstitutionEnabled = false
        editor.isAutomaticDashSubstitutionEnabled = false
        editor.setAccessibilityLabel("質問")
        editor.delegate = context.coordinator
        scroll.documentView = editor
        updateNSView(scroll, context: context)
        return scroll
    }

    func updateNSView(_ scroll: NSScrollView, context: Context) {
        context.coordinator.owner = self
        guard let editor = scroll.documentView as? ChatComposerTextView else { return }
        // Never replace the text storage while an input method owns marked text.
        if !editor.hasMarkedText(), editor.string != text {
            editor.string = text
            editor.needsDisplay = true
        }
        editor.isEditable = isEnabled
        editor.isSelectable = isEnabled
        editor.onSubmit = onSubmit
        editor.onEscape = onEscape
        if let scroll = scroll as? ChatComposerScrollView {
            scroll.focusOnAttachment = isFocused && isEnabled
        }
        if isFocused && isEnabled && !context.coordinator.requestedFocus {
            scroll.window?.makeFirstResponder(editor)
        }
        context.coordinator.requestedFocus = isFocused && isEnabled
    }

    func sizeThatFits(_ proposal: ProposedViewSize, nsView: NSScrollView, context: Context) -> CGSize? {
        guard let editor = nsView.documentView as? NSTextView,
              let container = editor.textContainer, let layout = editor.layoutManager else { return nil }
        let width = proposal.width ?? 300
        container.containerSize = NSSize(width: max(1, width - 8), height: CGFloat.greatestFiniteMagnitude)
        layout.ensureLayout(for: container)
        let lineHeight = layout.defaultLineHeight(for: editor.font ?? .systemFont(ofSize: 13))
        let contentHeight = layout.usedRect(for: container).height + layout.extraLineFragmentRect.height
        return CGSize(width: width, height: min(lineHeight * 6 + 8, max(lineHeight + 8, contentHeight + 8)))
    }

    final class Coordinator: NSObject, NSTextViewDelegate {
        var owner: ChatComposer
        var requestedFocus = false
        init(_ owner: ChatComposer) { self.owner = owner }

        func textDidChange(_ notification: Notification) {
            guard let editor = notification.object as? NSTextView else { return }
            owner.text = editor.string
            editor.needsDisplay = true
            editor.enclosingScrollView?.invalidateIntrinsicContentSize()
        }

        func textDidBeginEditing(_ notification: Notification) { owner.isFocused = true }
        func textDidEndEditing(_ notification: Notification) { owner.isFocused = false }
    }
}
