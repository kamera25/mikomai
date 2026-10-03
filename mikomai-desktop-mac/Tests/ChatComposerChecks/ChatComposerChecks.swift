import AppKit
import SwiftUI
import MikomaiDesktopCore

@main
struct ComposerChecks {
    @MainActor static func main() {
        _ = NSApplication.shared
        let editor = ChatComposerTextView(frame: NSRect(x: 0, y: 0, width: 600, height: 100))
        editor.isRichText = false
        var submissions = 0
        editor.onSubmit = { submissions += 1 }
        func press(_ modifiers: NSEvent.ModifierFlags = [], keyCode: UInt16 = 36) {
            let event = NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: modifiers,
                                        timestamp: 0, windowNumber: 0, context: nil,
                                        characters: "\r", charactersIgnoringModifiers: "\r",
                                        isARepeat: false, keyCode: keyCode)!
            editor.keyDown(with: event)
        }
        editor.string = "日本語"
        editor.setSelectedRange(NSRange(location: editor.string.utf16.count, length: 0))
        press()
        precondition(submissions == 1 && editor.string == "日本語", "Return submits without adding a newline")
        editor.setMarkedText("にほんご", selectedRange: NSRange(location: 4, length: 0), replacementRange: NSRange(location: NSNotFound, length: 0))
        precondition(editor.hasMarkedText())
        press()
        precondition(submissions == 1, "IME commit must not submit")
        // Keep marked text explicit: AppKit's event interpretation can depend
        // on the user's active input source. These checks must not depend on it.
        editor.setMarkedText("にほんご", selectedRange: NSRange(location: 4, length: 0), replacementRange: NSRange(location: NSNotFound, length: 0))
        press(.command)
        precondition(submissions == 1, "Command+Return during composition must not submit")
        editor.setMarkedText("にほんご", selectedRange: NSRange(location: 4, length: 0), replacementRange: NSRange(location: NSNotFound, length: 0))
        press(.shift)
        precondition(submissions == 1, "Shift+Return during composition must not submit")
        editor.unmarkText()
        press()
        precondition(submissions == 2, "Return after composition submits")
        press(.shift)
        precondition(submissions == 2 && editor.string.contains("\n"), "Shift+Return inserts newline")
        press(.command)
        precondition(submissions == 3, "Command+Return submits")
        press(keyCode: 76)
        precondition(submissions == 4, "Keypad Enter submits")
        editor.submitsOnReturn = false
        press()
        precondition(submissions == 4 && editor.string.hasSuffix("\n"), "Configuration editor Return inserts a newline without submitting")
        editor.submitsOnReturn = true
        editor.isEditable = false
        press()
        precondition(submissions == 4, "Disabled editor cannot submit")
        let completionEditor = ChatComposerTextView(frame: NSRect(x: 0, y: 0, width: 600, height: 100))
        completionEditor.isRichText = false
        var completionSubmissions = 0
        var completionKeys: [ChatSuggestionKey] = []
        completionEditor.onSubmit = { completionSubmissions += 1 }
        completionEditor.onSuggestionKey = { key in
            completionKeys.append(key)
            if key == .accept { completionEditor.completeMention(with: "router-01") }
            return true
        }
        func completionPress(_ keyCode: UInt16) {
            let event = NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: [],
                                        timestamp: 0, windowNumber: 0, context: nil,
                                        characters: "\r", charactersIgnoringModifiers: "\r",
                                        isARepeat: false, keyCode: keyCode)!
            completionEditor.keyDown(with: event)
        }
        func prepareMention() {
            completionEditor.string = "日本語😀 @rou を確認"
            completionEditor.setSelectedRange(NSRange(location: ("日本語😀 @rou" as NSString).length, length: 0))
        }
        prepareMention()
        let context = ChatMentionContext(text: completionEditor.string, selection: completionEditor.selectedRange())
        precondition(context?.query == "rou", "Mention query follows the caret, not the end of the draft")
        completionPress(125)
        completionPress(126)
        completionPress(36)
        precondition(completionKeys == [.next, .previous, .accept], "Arrow keys navigate and Enter accepts")
        precondition(completionEditor.string == "日本語😀 router-01  を確認", "Completion replaces only @query and preserves the suffix")
        precondition(completionSubmissions == 0, "Accepting a mention must not send the chat")
        precondition(completionEditor.selectedRange().location == ("日本語😀 router-01 " as NSString).length, "Caret follows the completion")
        prepareMention()
        completionPress(48)
        precondition(completionEditor.string.contains("router-01"), "Tab accepts a mention")
        prepareMention()
        let keysBeforeIME = completionKeys.count
        completionEditor.setMarkedText("るーた", selectedRange: NSRange(location: 3, length: 0), replacementRange: NSRange(location: NSNotFound, length: 0))
        completionPress(36)
        precondition(completionKeys.count == keysBeforeIME && completionSubmissions == 0, "IME Enter must not select a suggestion or send")
        completionEditor.unmarkText()
        prepareMention()
        completionPress(53)
        precondition(completionKeys.last == .dismiss, "Escape dismisses suggestions")
        precondition(completionEditor.string == "日本語😀 @rou を確認", "Dismissing suggestions preserves the draft")
        precondition(ChatMentionContext(text: "@router test", selection: NSRange(location: 12, length: 0)) == nil, "Whitespace ends a mention")
        precondition(ChatMentionContext(text: "hello", selection: NSRange(location: 5, length: 0)) == nil, "Ordinary text has no mention")
        precondition(ChatMentionContext(text: "@one @two", selection: NSRange(location: 9, length: 0))?.query == "two", "The nearest @ is used")
        precondition(ChatMentionContext(text: "@one", selection: NSRange(location: 1, length: 2)) == nil, "A selection is not an active mention")
        completionEditor.onSuggestionKey = { _ in false }
        completionPress(36)
        precondition(completionSubmissions == 1, "Without visible suggestions Enter sends normally")
        var boundDraft = ""
        var reportedContext: ChatMentionContext?
        let composer = ChatComposer(
            text: Binding(get: { boundDraft }, set: { boundDraft = $0 }),
            isFocused: .constant(true), isEnabled: true, onSubmit: {}, onEscape: {},
            onMentionContextChanged: { reportedContext = $0 }
        )
        let coordinator = composer.makeCoordinator()
        let boundEditor = ChatComposerTextView(frame: NSRect(x: 0, y: 0, width: 600, height: 100))
        boundEditor.isRichText = false
        boundEditor.delegate = coordinator
        boundEditor.onMentionContextChanged = composer.onMentionContextChanged
        boundEditor.insertText("確認 @local", replacementRange: NSRange(location: 0, length: 0))
        precondition(boundDraft == "確認 @local", "Native editing updates the SwiftUI draft binding")
        precondition(reportedContext?.query == "local", "Native editing reports the active mention to the suggestion list")
        let registryJSON = Data(#"[{"hostname":"edge-tauri","ip":"192.0.2.10","password":"ignored"},{"hostname":"console","ip":null}]"#.utf8)
        let registryHosts = try! HostCompletionSource.decode(registryJSON)
        precondition(registryHosts == [HostSuggestion(hostname: "edge-tauri", ip: "192.0.2.10"), HostSuggestion(hostname: "console", ip: "Console")])
        let nativeHosts = [HostSuggestion(hostname: "edge-tauri", ip: "192.0.2.20")]
        let mergedHosts = HostCompletionSource.merge(registry: registryHosts, native: nativeHosts)
        precondition(mergedHosts.first?.ip == "192.0.2.20" && mergedHosts.count == 2, "Native metadata wins without losing registry hosts")
        precondition(HostSuggestionPolicy.find(query: "edge", availableHosts: registryHosts, recentIPs: [], labels: HostSuggestionLabels(localhost: "このコンピュータ", pastIps: "最近のIP")).first?.hostname == "edge-tauri", "legacy registry hosts are searchable without importing the native inventory")
        precondition(ChatMentionContext(text: "＠ｌｏｃ", selection: NSRange(location: 4, length: 0))?.query == "loc", "Japanese full-width @ and ASCII queries are supported")
        let labels = HostSuggestionLabels(localhost: "このコンピュータ", pastIps: "最近のIP")
        precondition(HostSuggestionPolicy.find(query: "", availableHosts: [], recentIPs: [], labels: labels).map(\.hostname) == ["localhost"], "Bare @ must show localhost even without registered devices")
        precondition(HostSuggestionPolicy.find(query: "", availableHosts: registryHosts, recentIPs: ["198.51.100.1"], labels: labels).map(\.hostname) == ["localhost", "edge-tauri", "console", "198.51.100.1"], "Bare @ shows all registered hosts and recent IPs")
        checkTabNavigation()
        checkHostedSuggestions()
        print("PASS: cursor-aware @ completion, Unicode and suffix preservation, arrow/Enter/Tab/Escape routing, IME priority and normal-send fallback")
        print("PASS: Return submission, IME Return/Command+Return/Shift+Return suppression, post-composition submission, Shift+Return newline, Command+Return, keypad Enter, disabled editor")
    }
}

@MainActor
private final class SuggestionDisplayProbe: ObservableObject {
    @Published var candidateCount = 0
    var visible = false
    var context: ChatMentionContext?
    var submissions = 0
}

private struct SuggestionDisplayHost: View {
    @ObservedObject var probe: SuggestionDisplayProbe
    @State private var text = ""
    @State private var focused = true
    @State private var presentation = ChatMentionPresentation()
    @State private var completion: ChatMentionCompletion?

    var body: some View {
        let visible = presentation.isVisible(candidateCount: probe.candidateCount)
        let _ = { probe.visible = visible; probe.context = presentation.context }()
        VStack {
            if visible { Text("localhost") }
            ChatComposer(text: $text, isFocused: $focused, isEnabled: true,
                         onSubmit: { probe.submissions += 1 }, onEscape: { presentation.dismiss() },
                         onSuggestionKey: { key in
                             guard presentation.isVisible(candidateCount: probe.candidateCount) else { return false }
                             if key == .dismiss { presentation.dismiss() }
                             if key == .accept {
                                 presentation.dismiss()
                                 completion = ChatMentionCompletion(hostname: "localhost")
                             }
                             return true
                         }, onMentionContextChanged: { presentation.update(context: $0) }, completion: completion)
        }
        .frame(width: 600, height: 200)
    }
}

@MainActor
private func checkHostedSuggestions() {
    let probe = SuggestionDisplayProbe()
    let hosting = NSHostingView(rootView: SuggestionDisplayHost(probe: probe))
    let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 600, height: 200),
                          styleMask: [.titled], backing: .buffered, defer: false)
    window.contentView = hosting
    hosting.layoutSubtreeIfNeeded()
    func pump() { RunLoop.main.run(until: Date().addingTimeInterval(0.15)) }
    func findEditor(_ view: NSView) -> ChatComposerTextView? {
        if let editor = view as? ChatComposerTextView { return editor }
        return view.subviews.lazy.compactMap { findEditor($0) }.first
    }
    func event(_ key: UInt16) -> NSEvent {
        NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: [], timestamp: 0,
                         windowNumber: 0, context: nil, characters: "\r", charactersIgnoringModifiers: "\r",
                         isARepeat: false, keyCode: key)!
    }
    pump()
    let editor = findEditor(hosting)!
    editor.insertText("@", replacementRange: NSRange(location: 0, length: 0))
    pump()
    precondition(probe.context?.query == "" && !probe.visible, "The mounted SwiftUI component reports @ before hosts arrive")
    probe.candidateCount = 1
    pump()
    precondition(probe.visible, "Candidates arriving after @ must become visible without further typing")
    editor.keyDown(with: event(53))
    pump()
    precondition(!probe.visible && editor.string == "@", "Escape closes the mounted list without changing text")
    probe.candidateCount = 2
    pump()
    precondition(!probe.visible, "Host refresh must not reopen an explicitly dismissed list")
    editor.insertText("l", replacementRange: editor.selectedRange())
    pump()
    precondition(probe.visible && probe.context?.query == "l", "Editing the mention reopens suggestions")
    editor.keyDown(with: event(36))
    pump()
    precondition(editor.string == "localhost " && probe.submissions == 0 && !probe.visible,
                 "Enter accepts the mounted list and completes the SwiftUI/AppKit round trip without sending")
    editor.insertText("＠ｌｏｃ", replacementRange: NSRange(location: 0, length: (editor.string as NSString).length))
    pump()
    precondition(probe.visible && probe.context?.query == "loc", "Full-width Japanese mentions open the mounted list")
    editor.setMarkedText("＠", selectedRange: NSRange(location: 1, length: 0),
                         replacementRange: NSRange(location: 0, length: (editor.string as NSString).length))
    pump()
    precondition(!probe.visible, "IME marked text must not expose selectable suggestions")
    editor.unmarkText()
    pump()
    precondition(probe.visible && probe.context?.query == "", "Committing Japanese @ refreshes the mounted list")
    print("PASS: mounted SwiftUI candidate display, asynchronous host arrival, Escape persistence, completion round trip, Japanese full-width input and IME commit")

    // Verify file drag and drop interception on ChatComposerTextView
    let dragEditor = ChatComposerTextView(frame: NSRect(x: 0, y: 0, width: 600, height: 100))
    var droppedURLs: [URL] = []
    var isDragTargeted = false
    dragEditor.onFileDrop = { urls in
        droppedURLs = urls
        return true
    }
    dragEditor.onDragTargetChanged = { targeted in
        isDragTargeted = targeted
    }

    final class MockDraggingInfo: NSObject, NSDraggingInfo {
        let pasteboard: NSPasteboard
        init(pasteboard: NSPasteboard) {
            self.pasteboard = pasteboard
        }
        var draggingPasteboard: NSPasteboard { pasteboard }
        var draggingDestinationWindow: NSWindow? { nil }
        var draggingSourceOperationMask: NSDragOperation { .copy }
        var draggingLocation: NSPoint { .zero }
        var draggedImageLocation: NSPoint { .zero }
        var draggedImage: NSImage? { nil }
        var draggingSource: Any? { nil }
        var draggingSequenceNumber: Int { 1 }
        func slideDraggedImage(to screenPoint: NSPoint) {}
        override func namesOfPromisedFilesDropped(atDestination dropDestination: URL) -> [String]? { nil }
        var draggingFormation: NSDraggingFormation = .default
        var animatesToDestination: Bool = false
        var numberOfValidItemsForDrop: Int = 1
        var springLoadingHighlight: NSSpringLoadingHighlight { .none }
        func resetSpringLoading() {}
        func enumerateDraggingItems(options: NSDraggingItemEnumerationOptions = [], for view: NSView?, classes: [AnyClass], searchOptions: [NSPasteboard.ReadingOptionKey : Any] = [:], using block: (NSDraggingItem, Int, UnsafeMutablePointer<ObjCBool>) -> Void) {}
    }

    let pb = NSPasteboard.withUniqueName()
    let testURL = URL(fileURLWithPath: "/tmp/sample_config.txt")
    pb.writeObjects([testURL as NSURL])
    let dragInfo = MockDraggingInfo(pasteboard: pb)

    let enterOp = dragEditor.draggingEntered(dragInfo)
    precondition(enterOp == .copy, "Dragging entered with file URL must return .copy")
    precondition(isDragTargeted, "Drag target state must be true on draggingEntered")

    dragEditor.draggingExited(dragInfo)
    precondition(!isDragTargeted, "Drag target state must be false on draggingExited")

    _ = dragEditor.draggingEntered(dragInfo)
    let dropSuccess = dragEditor.performDragOperation(dragInfo)
    precondition(dropSuccess, "performDragOperation must succeed")
    precondition(droppedURLs == [testURL], "onFileDrop must receive the dropped URL")
    precondition(!isDragTargeted, "Drag target state must be false after performDragOperation")
    print("PASS: ChatComposerTextView file drag & drop interception")
}


@MainActor
private func checkTabNavigation() {
    let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 600, height: 200),
                          styleMask: [.titled], backing: .buffered, defer: false)
    let previous = NSTextField(string: "前の項目")
    let next = NSTextField(string: "次の項目")
    let editor = ChatComposerTextView(frame: NSRect(x: 0, y: 50, width: 400, height: 80))
    editor.isRichText = false
    editor.string = "確認 @rou"
    previous.frame = NSRect(x: 0, y: 150, width: 200, height: 24)
    next.frame = NSRect(x: 0, y: 10, width: 200, height: 24)
    for view in [previous, editor, next] { window.contentView!.addSubview(view) }
    previous.nextKeyView = editor
    editor.nextKeyView = next
    next.nextKeyView = previous
    func tab(_ modifiers: NSEvent.ModifierFlags = []) {
        editor.keyDown(with: NSEvent.keyEvent(with: .keyDown, location: .zero,
            modifierFlags: modifiers, timestamp: 0, windowNumber: window.windowNumber,
            context: nil, characters: "\t", charactersIgnoringModifiers: "\t",
            isARepeat: false, keyCode: 48)!)
    }
    window.makeFirstResponder(editor)
    tab()
    precondition(next.currentEditor() === window.firstResponder,
                 "Tab moves from the multiline composer to the next field")
    precondition(editor.string == "確認 @rou", "Tab navigation never changes the draft")
    window.makeFirstResponder(editor)
    var accepted = false
    editor.onSuggestionKey = { _ in accepted = true; return true }
    tab(.shift)
    precondition(previous.currentEditor() === window.firstResponder && !accepted,
                 "Shift+Tab moves backwards without accepting an open suggestion")
    window.makeFirstResponder(editor)
    tab()
    precondition(accepted && window.firstResponder === editor,
                 "Unmodified Tab still accepts an open suggestion without leaving the editor")
    editor.onSuggestionKey = { _ in false }
    editor.setMarkedText("るーた", selectedRange: NSRange(location: 3, length: 0),
                         replacementRange: NSRange(location: NSNotFound, length: 0))
    tab()
    precondition(window.firstResponder === editor, "IME Tab stays with the input method")
    print("PASS: Tab/Shift+Tab focus traversal, draft preservation, suggestion priority and IME Tab")
}
