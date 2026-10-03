import AppKit
import SwiftUI

enum ChatSuggestionKey: Equatable { case next, previous, accept, dismiss }

struct ChatMentionCompletion {
    let id = UUID()
    let hostname: String
}

struct ChatMentionContext: Equatable {
    let query: String
    let range: NSRange

    init?(text: String, selection: NSRange) {
        let source = text as NSString
        guard selection.length == 0, selection.location <= source.length else { return nil }
        let prefix = source.substring(to: selection.location) as NSString
        let at = prefix.rangeOfCharacter(from: CharacterSet(charactersIn: "@＠"), options: .backwards)
        guard at.location != NSNotFound else { return nil }
        let query = prefix.substring(from: at.location + 1)
        guard !query.contains(where: \.isWhitespace) else { return nil }
        // Japanese input can commit full-width ASCII. Keep the original range
        // for replacement, but search using the equivalent half-width query.
        self.query = query.unicodeScalars.map { scalar in
            (0xFF01...0xFF5E).contains(scalar.value)
                ? String(Unicode.Scalar(scalar.value - 0xFEE0)!) : String(scalar)
        }.joined()
        range = NSRange(location: at.location, length: selection.location - at.location)
    }
}

struct ChatMentionPresentation {
    private(set) var context: ChatMentionContext?
    private var dismissedContext: ChatMentionContext?

    mutating func update(context: ChatMentionContext?) {
        guard self.context != context else { return }
        self.context = context
        dismissedContext = nil
    }

    mutating func dismiss() { dismissedContext = context }

    func isVisible(candidateCount: Int) -> Bool {
        context != nil && context != dismissedContext && candidateCount > 0
    }
}

/// Checks the composition state before AppKit consumes the Return event.
/// Checking afterwards would mistake the Return that commits Japanese text
/// for a request to send that text.
final class ChatComposerTextView: NSTextView {
    var onSubmit: () -> Void = {}
    var onEscape: () -> Void = {}
    var onSuggestionKey: (ChatSuggestionKey) -> Bool = { _ in false }
    var onMentionContextChanged: (ChatMentionContext?) -> Void = { _ in }
    var onFileDrop: (([URL]) -> Bool)? = nil
    var onDragTargetChanged: ((Bool) -> Void)? = nil

    private func extractFileURLs(from pasteboard: NSPasteboard) -> [URL] {
        if let urls = pasteboard.readObjects(forClasses: [NSURL.self], options: [
            .urlReadingFileURLsOnly: true
        ]) as? [URL], !urls.isEmpty {
            return urls
        }
        if let filenames = pasteboard.propertyList(forType: .init("NSFilenamesPboardType")) as? [String] {
            return filenames.map { URL(fileURLWithPath: $0) }
        }
        return []
    }

    override func draggingEntered(_ sender: NSDraggingInfo) -> NSDragOperation {
        if onFileDrop != nil && !extractFileURLs(from: sender.draggingPasteboard).isEmpty {
            onDragTargetChanged?(true)
            return .copy
        }
        return super.draggingEntered(sender)
    }

    override func draggingUpdated(_ sender: NSDraggingInfo) -> NSDragOperation {
        if onFileDrop != nil && !extractFileURLs(from: sender.draggingPasteboard).isEmpty {
            return .copy
        }
        return super.draggingUpdated(sender)
    }

    override func draggingExited(_ sender: NSDraggingInfo?) {
        onDragTargetChanged?(false)
        super.draggingExited(sender)
    }

    override func draggingEnded(_ sender: NSDraggingInfo) {
        onDragTargetChanged?(false)
        super.draggingEnded(sender)
    }

    override func performDragOperation(_ sender: NSDraggingInfo) -> Bool {
        onDragTargetChanged?(false)
        if let onFileDrop, !extractFileURLs(from: sender.draggingPasteboard).isEmpty {
            let urls = extractFileURLs(from: sender.draggingPasteboard)
            if !urls.isEmpty {
                return onFileDrop(urls)
            }
        }
        return super.performDragOperation(sender)
    }

    func scheduleMentionReport() {
        DispatchQueue.main.async { [weak self] in self?.reportMentionQuery() }
    }

    override func didChangeText() {
        super.didChangeText()
        // NSTextView delegates can be notified before the input method finishes
        // committing text and moving the caret. Read both on the next run loop.
        scheduleMentionReport()
    }

    override func unmarkText() {
        super.unmarkText()
        scheduleMentionReport()
    }

    func reportMentionQuery() {
        onMentionContextChanged(hasMarkedText() ? nil : ChatMentionContext(text: string, selection: selectedRange()))
    }

    func completeMention(with hostname: String) {
        guard isEditable, !hasMarkedText(),
              let context = ChatMentionContext(text: string, selection: selectedRange()) else { return }
        insertText(hostname + " ", replacementRange: context.range)
        reportMentionQuery()
    }

    override func keyDown(with event: NSEvent) {
        let modifiers = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
        // The input method gets first refusal during Japanese composition.
        if !hasMarkedText(), isEditable {
            let key: ChatSuggestionKey? = switch event.keyCode {
            case 125: .next
            case 126: .previous
            case 36, 76, 48: .accept
            case 53: .dismiss
            default: nil
            }
            if let key, onSuggestionKey(key) { return }
        }
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
                             withAttributes: [.font: font ?? NSFont.systemFont(ofSize: 15),
                                              .foregroundColor: NSColor.placeholderTextColor])
        }
    }
}

final class ChatComposerScrollView: NSScrollView {
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
    var onSuggestionKey: (ChatSuggestionKey) -> Bool = { _ in false }
    var onMentionContextChanged: (ChatMentionContext?) -> Void = { _ in }
    var completion: ChatMentionCompletion? = nil
    var onFileDrop: (([URL]) -> Bool)? = nil
    var onDragTargetChanged: ((Bool) -> Void)? = nil

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
        editor.font = .systemFont(ofSize: 15)
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
            editor.setSelectedRange(NSRange(location: (text as NSString).length, length: 0))
            editor.scheduleMentionReport()
            editor.needsDisplay = true
        }
        editor.isEditable = isEnabled
        editor.isSelectable = isEnabled
        editor.onSubmit = onSubmit
        editor.onEscape = onEscape
        editor.onSuggestionKey = onSuggestionKey
        editor.onMentionContextChanged = onMentionContextChanged
        editor.onFileDrop = onFileDrop
        editor.onDragTargetChanged = onDragTargetChanged
        if let completion, context.coordinator.lastCompletionID != completion.id {
            context.coordinator.lastCompletionID = completion.id
            DispatchQueue.main.async { [weak editor, weak coordinator = context.coordinator] in
                guard let editor, coordinator?.owner.completion?.id == completion.id else { return }
                scroll.window?.makeFirstResponder(editor)
                editor.completeMention(with: completion.hostname)
            }
        }
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
        let lineHeight = layout.defaultLineHeight(for: editor.font ?? .systemFont(ofSize: 15))
        let contentHeight = layout.usedRect(for: container).height + layout.extraLineFragmentRect.height
        return CGSize(width: width, height: min(lineHeight * 6 + 8, max(lineHeight + 8, contentHeight + 8)))
    }

    final class Coordinator: NSObject, NSTextViewDelegate {
        var owner: ChatComposer
        var requestedFocus = false
        var lastCompletionID: UUID?
        init(_ owner: ChatComposer) { self.owner = owner }

        func textDidChange(_ notification: Notification) {
            guard let editor = notification.object as? ChatComposerTextView else { return }
            owner.text = editor.string
            editor.reportMentionQuery()
            editor.needsDisplay = true
            editor.enclosingScrollView?.invalidateIntrinsicContentSize()
        }

        func textDidBeginEditing(_ notification: Notification) { owner.isFocused = true }
        func textDidEndEditing(_ notification: Notification) { owner.isFocused = false }

        func textViewDidChangeSelection(_ notification: Notification) {
            guard let editor = notification.object as? ChatComposerTextView else { return }
            // Programmatic storage updates can notify during SwiftUI rendering.
            DispatchQueue.main.async { [weak editor] in editor?.reportMentionQuery() }
        }
    }
}
