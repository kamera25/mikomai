import AppKit

@main
struct DebugTerminalChecks {
    @MainActor static func main() {
        let scroll = CoreDebugTerminal.makeEditor()
        scroll.frame = NSRect(x: 0, y: 0, width: 320, height: 200)
        scroll.layoutSubtreeIfNeeded()
        let text = (0..<150).map { "line \($0): " + String(repeating: "x", count: 100) }.joined(separator: "\n")
        CoreDebugTerminal.update(scroll, text: text, followsOutput: false)
        guard let editor = scroll.documentView as? NSTextView else { fatalError("missing editor") }
        assert(!editor.isEditable && editor.isSelectable)
        scroll.contentView.scroll(to: NSPoint(x: 100, y: 120))
        let oldY = scroll.contentView.bounds.origin.y
        CoreDebugTerminal.update(scroll, text: text + "\nappended", followsOutput: true)
        assert(abs(scroll.contentView.bounds.origin.y - oldY) < 1, "reading position must survive append")
        assert(scroll.contentView.bounds.origin.x == 0, "updated log must start at left edge")
        scroll.contentView.scroll(to: NSPoint(x: 100, y: editor.frame.height - scroll.contentView.bounds.height))
        CoreDebugTerminal.update(scroll, text: text + "\nappended\nnew tail", followsOutput: true)
        assert(abs(scroll.contentView.bounds.maxY - editor.frame.height) < 1, "follow at end")
        CoreDebugTerminal.update(scroll, text: "", followsOutput: false)
        assert(scroll.contentView.bounds.origin == .zero, "clear resets viewport")
        print("Debug terminal: viewport preservation, left alignment, tail follow and clear passed")
    }
}
