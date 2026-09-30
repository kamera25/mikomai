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
 _ = app
 }
}
