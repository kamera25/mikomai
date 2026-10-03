import MikomaiDesktopCore

@main struct FullCheck {
 @MainActor static func main() {
 setenv("MIKOMAI_SETTINGS_PATH", "/private/tmp/mikomai-full-check/settings.json", 1)
 let app = NSApplication.shared
 let model = DesktopModel()
 let host = NSHostingView(rootView: DesktopWindow(model:model))
 let window=NSWindow(contentRect:NSRect(x:0,y:0,width:1200,height:800),styleMask:[.titled],backing:.buffered,defer:false)
 window.contentView=host
 func pump(){RunLoop.main.run(until:Date().addingTimeInterval(0.5));host.layoutSubtreeIfNeeded()}
 func editor(_ view:NSView)->ChatComposerTextView? { if let e=view as? ChatComposerTextView{return e};return view.subviews.lazy.compactMap{editor($0)}.first }
 pump()
 func mainSplit(_ view: NSView) -> NSSplitView? {
     if let split = view as? NSSplitView, split.isVertical, split.subviews.count > 1 { return split }
     return view.subviews.lazy.compactMap { mainSplit($0) }.first
 }
 guard let split = mainSplit(host) else { fatalError("expected history split view") }
 precondition(abs(split.subviews[0].frame.width - 248) <= 1,
              "History pane must start at the reference screenshot's 248pt width")
 split.setPosition(300, ofDividerAt: 0)
 model.draft = "resize check"
 pump()
 precondition(abs(split.subviews[0].frame.width - 300) <= 1,
              "View updates must preserve manual history pane resizing")
 model.draft = ""
 split.setPosition(248, ofDividerAt: 0)
 pump()
 print("PASS: history pane starts at 248pt and preserves manual resizing")
 guard let e=editor(host) else {fatalError("no editor")}
 window.makeFirstResponder(e)
 e.insertText("@",replacementRange:NSRange(location:NSNotFound,length:0))
 pump()
 let bitmap=host.bitmapImageRepForCachingDisplay(in:host.bounds)!
 host.cacheDisplay(in:host.bounds,to:bitmap)
 try! bitmap.representation(using:.png,properties:[:])!.write(to:URL(fileURLWithPath:"/private/tmp/mikomai-full-check/window.png"))
 precondition(model.draft == "@")
 let enter = NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: [], timestamp: 0, windowNumber: window.windowNumber, context: nil, characters: "\r", charactersIgnoringModifiers: "\r", isARepeat: false, keyCode: 36)!
 e.keyDown(with: enter)
 pump()
 precondition(e.string == "localhost " && model.draft == "localhost ", "Bare @ must render a selectable candidate in the production window")
 model.draft = ""
 pump()
 e.insertText("＠", replacementRange: NSRange(location: NSNotFound, length: 0))
 pump()
 e.keyDown(with: enter)
 pump()
 precondition(e.string == "localhost " && model.draft == "localhost ", "Full-width @ must render a selectable candidate in the production window")
 print("PASS: production DesktopWindow half/full-width @ candidate selection")

 // Verify chat history session row with truncated long title
 guard let initialSessionID = model.activeSessionID else { fatalError("expected active session") }
 let longTitle = "非常に長いチャットセッションのタイトルです。F220のVLAN設定方法とTrunkポート設定について詳しく教えてください。"
 model.renameSession(initialSessionID, title: longTitle)
 pump()
 precondition(model.sessions.first(where: { $0.id == initialSessionID })?.title == longTitle)

 // Verify creating a second session and switching between them via model
 model.createSession()
 let secondSessionID = model.activeSessionID
 precondition(secondSessionID != initialSessionID)
 pump()
 model.select(initialSessionID)
 precondition(model.activeSessionID == initialSessionID)
 pump()

 // Verify HoverScrollTitle view mounts and computes truncation correctly
 let testHost = NSHostingView(rootView: HoverScrollTitle(title: longTitle, isHovered: true))
 testHost.frame = NSRect(x: 0, y: 0, width: 200, height: 36)
 window.contentView?.addSubview(testHost)
 pump()
 testHost.removeFromSuperview()
 print("PASS: chat history session row with long title and HoverScrollTitle mounting")

 // Verify file attachment via attachFiles (supporting drag & drop)
 let tempDir = FileManager.default.temporaryDirectory
 let testFile = tempDir.appendingPathComponent("vlan_config.txt")
 try! "vlan 10\n name SALES\n".write(to: testFile, atomically: true, encoding: .utf8)
 defer { try? FileManager.default.removeItem(at: testFile) }

 let attached = model.attachFiles(at: [testFile])
 precondition(attached, "attachFiles must succeed for valid txt file")
 precondition(model.pendingAttachments.count == 1, "pendingAttachments must contain 1 attachment")
 precondition(model.pendingAttachments.first?.name == "vlan_config.txt")
 precondition(model.attachmentError.isEmpty, "attachmentError must be empty on success")
 pump()

 // Verify removing attachment
 if let attachmentID = model.pendingAttachments.first?.id {
     model.removeAttachment(attachmentID)
     precondition(model.pendingAttachments.isEmpty, "pendingAttachments must be empty after removeAttachment")
 }
 pump()

 // Verify unsupported file rejection
 let unsupportedFile = tempDir.appendingPathComponent("invalid.exe")
 try! "binary content".write(to: unsupportedFile, atomically: true, encoding: .utf8)
 defer { try? FileManager.default.removeItem(at: unsupportedFile) }
 let rejected = model.attachFiles(at: [unsupportedFile])
 precondition(!rejected, "attachFiles must return false for unsupported file")
 precondition(!model.attachmentError.isEmpty, "attachmentError must be set for unsupported file")
 precondition(model.pendingAttachments.isEmpty, "unsupported file must not be attached")
 pump()
 print("PASS: file attachment and drag-and-drop validation in chat window")

 // Resize the production chat after laying it out at full width, as when snapping.
 let message = ChatMessage(role: .user, text: String(repeating: "Yamahaで設定変更するコマンドを教えて。", count: 12),
                           attachments: [String(repeating: "長い添付ファイル名", count: 8) + ".txt"])
 let shortMessage = ChatMessage(role: .user, text: "FitelnetのVLAN設定を教えて")
 model.sessions = [ChatSession(id: initialSessionID, title: "折り返し確認", messages: [shortMessage, message])]
 model.activeSessionID = initialSessionID
 func observation(_ view: NSView) -> ChatScrollObservationView? {
     if let result = view as? ChatScrollObservationView { return result }
     return view.subviews.lazy.compactMap { observation($0) }.first
 }
 for width in [1200.0, 740.0, 600.0, 1200.0] {
     window.setContentSize(NSSize(width: width, height: 800))
     pump()
     pump()
     guard let observer = observation(host), let scroll = observer.enclosingScrollView,
           let document = scroll.documentView else { fatalError("expected chat scroll view") }
     precondition(document.bounds.width <= scroll.contentView.bounds.width + 1,
                  "Chat content must fit the viewport after resizing to \(width)")
     let viewport = scroll.convert(scroll.bounds, to: host)
     FileHandle.standardError.write(Data("CHAT width=\(width) host=\(host.bounds.width) viewport=\(viewport) content=\(observer.bounds)\n".utf8))
     precondition(viewport.maxX <= host.bounds.maxX + 1, "Chat viewport must fit the window")
     let image = host.bitmapImageRepForCachingDisplay(in: host.bounds)!
     host.cacheDisplay(in: host.bounds, to: image)
     try! image.representation(using: .png, properties: [:])!.write(
         to: URL(fileURLWithPath: "/private/tmp/mikomai-full-check/chat-\(Int(width)).png"))
 }
 let wideSize = NSHostingView(rootView: MessageRow(message: message).frame(width: 760)).fittingSize
 let narrowSize = NSHostingView(rootView: MessageRow(message: message).frame(width: 350)).fittingSize
 precondition(narrowSize.width <= 351, "User message must fit a narrow row")
 precondition(narrowSize.height > wideSize.height, "Long user text and attachment names must wrap vertically")
 print("PASS: resized production chat fits viewport; user text and attachments wrap")

 // Exercise the native editor used by chat history titles.
 for fontSize in [14.0] {
     var savedTitle = "元のチャットタイトル"
     var cancelled = false
     let titleHost = NSHostingView(rootView: ChatTitleEditor(
         title: savedTitle, fontSize: fontSize, weight: .regular,
         onCommit: { savedTitle = $0 }, onCancel: { cancelled = true }))
     titleHost.frame = NSRect(x: 0, y: 0, width: 300, height: 36)
     window.contentView?.addSubview(titleHost)
     pump()
     func titleField(_ view: NSView) -> ChatTitleEditor.TitleField? {
         if let field = view as? ChatTitleEditor.TitleField { return field }
         return view.subviews.lazy.compactMap { titleField($0) }.first
     }
     guard let field = titleField(titleHost), let input = field.currentEditor() as? NSTextView else {
         fatalError("Title editor must receive focus as soon as it appears")
     }
     precondition(input.selectedRange() == NSRange(location: 0, length: (savedTitle as NSString).length),
                  "The whole title must be selected when editing starts")
     input.insertText("破棄するタイトル", replacementRange: input.selectedRange())
     let escape = NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: [], timestamp: 0,
                                  windowNumber: window.windowNumber, context: nil, characters: "\u{1b}",
                                  charactersIgnoringModifiers: "\u{1b}", isARepeat: false, keyCode: 53)!
     input.keyDown(with: escape)
     pump()
     precondition(cancelled && savedTitle == "元のチャットタイトル", "Esc must cancel without saving")
     titleHost.removeFromSuperview()

     let reopened = NSHostingView(rootView: ChatTitleEditor(
         title: savedTitle, fontSize: fontSize, weight: .regular,
         onCommit: { savedTitle = $0 }, onCancel: {}))
     reopened.frame = NSRect(x: 0, y: 0, width: 300, height: 36)
     window.contentView?.addSubview(reopened)
     pump()
     guard let reopenedField = titleField(reopened), let reopenedInput = reopenedField.currentEditor() as? NSTextView else {
         fatalError("Reopened title editor must receive focus")
     }
     precondition(reopenedInput.string == "元のチャットタイトル", "Cancelled edits must be discarded")
     precondition(reopenedInput.selectedRange().length == (savedTitle as NSString).length)
     reopenedInput.insertText("新しいタイトル", replacementRange: reopenedInput.selectedRange())
     reopenedInput.keyDown(with: enter)
     pump()
     precondition(savedTitle == "新しいタイトル", "Enter must save the edited title")
     reopened.removeFromSuperview()
 }
 print("PASS: history title editor focuses, selects all, cancels with Esc, and saves with Enter")

 _ = app
 }
}
