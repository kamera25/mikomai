import AppKit

@MainActor
public protocol ChatClipboardBoard {
    func clearForCopy()
    @discardableResult func setString(_ string: String, forType type: NSPasteboard.PasteboardType) -> Bool
}

extension NSPasteboard: ChatClipboardBoard {
    public func clearForCopy() { _ = clearContents() }
}

@MainActor
public enum ChatMessageClipboard {
    @discardableResult
    public static func copy(text: String, to pasteboard: any ChatClipboardBoard = NSPasteboard.general) -> Bool {
        pasteboard.clearForCopy()
        return pasteboard.setString(text, forType: .string)
    }
}
