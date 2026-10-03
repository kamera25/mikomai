import AppKit
import SwiftUI

/// Manages the chat history title's edit lifecycle.
struct EditableChatTitle<Display: View>: View {
    let title: String
    var fontSize: CGFloat = 14
    var weight: NSFont.Weight = .regular
    let onRename: (String) -> Void
    @ViewBuilder let display: (@escaping () -> Void) -> Display

    @State private var isEditing = false

    var body: some View {
        if isEditing {
            ChatTitleEditor(title: title, fontSize: fontSize, weight: weight, onCommit: {
                onRename($0)
                isEditing = false
            }, onCancel: {
                isEditing = false
            })
        } else {
            display { isEditing = true }
        }
    }
}

struct ChatTitleEditor: NSViewRepresentable {
    let title: String
    let fontSize: CGFloat
    let weight: NSFont.Weight
    let onCommit: (String) -> Void
    let onCancel: () -> Void

    final class TitleField: NSTextField {
        private var didSelectInitialTitle = false

        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            guard window != nil, !didSelectInitialTitle else { return }
            DispatchQueue.main.async { [weak self] in
                guard let self, let window = self.window,
                      !self.didSelectInitialTitle,
                      window.makeFirstResponder(self) else { return }
                self.selectText(nil)
                self.didSelectInitialTitle = true
            }
        }
    }

    final class Coordinator: NSObject, NSTextFieldDelegate {
        var parent: ChatTitleEditor
        init(_ parent: ChatTitleEditor) { self.parent = parent }

        func control(_ control: NSControl, textView: NSTextView,
                     doCommandBy commandSelector: Selector) -> Bool {
            switch commandSelector {
            case #selector(NSResponder.insertNewline(_:)):
                parent.onCommit(textView.string)
                return true
            case #selector(NSResponder.cancelOperation(_:)):
                parent.onCancel()
                return true
            default:
                return false
            }
        }
    }

    func makeCoordinator() -> Coordinator { Coordinator(self) }

    func makeNSView(context: Context) -> TitleField {
        let field = TitleField(string: title)
        field.isBordered = false
        field.drawsBackground = false
        field.font = .systemFont(ofSize: fontSize, weight: weight)
        field.lineBreakMode = .byTruncatingTail
        field.maximumNumberOfLines = 1
        field.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        field.delegate = context.coordinator
        field.setAccessibilityLabel("会話名")
        return field
    }

    func updateNSView(_ nsView: TitleField, context: Context) {
        context.coordinator.parent = self
    }
}
