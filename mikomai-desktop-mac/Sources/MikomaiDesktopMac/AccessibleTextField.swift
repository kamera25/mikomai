import AppKit
import SwiftUI

/// Visible labels and native AX names persist after entering text. Secure
/// fields keep AppKit's protected value instead of publishing a custom value.
struct AccessibleTextField: View {
    let title: String
    @Binding var text: String
    var help: String = ""
    var isSecure = false
    var placeholder = ""
    var defersTabNavigation = false

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(title).font(.system(size: 13)).accessibilityHidden(true)
            NativeField(title: title, text: $text, help: help, isSecure: isSecure, placeholder: placeholder, defersTabNavigation: defersTabNavigation)
                .frame(minHeight: 26)
        }
    }

    private struct NativeField: NSViewRepresentable {
        let title: String
        @Binding var text: String
        let help: String
        let isSecure: Bool
        let placeholder: String
        let defersTabNavigation: Bool
        @Environment(\.isEnabled) private var isEnabled

        func makeCoordinator() -> Coordinator { Coordinator(self) }
        func makeNSView(context: Context) -> NSTextField {
            let field: NSTextField = isSecure ? AccessibleSecureField() : AccessibleInputField()
            field.isEditable = true
            field.isSelectable = true
            field.isBezeled = true
            field.bezelStyle = .roundedBezel
            field.font = .systemFont(ofSize: 14)
            field.delegate = context.coordinator
            field.stringValue = text
            return field
        }
        func updateNSView(_ field: NSTextField, context: Context) {
            context.coordinator.owner = self
            field.isEnabled = isEnabled
            (field as? AccessibleInputField)?.defersTabNavigation = defersTabNavigation
            field.placeholderString = placeholder
            field.setAccessibilityLabel(title)
            field.setAccessibilityHelp(help)
            // Binding updates must not replace marked text or reset the cursor.
            if field.stringValue != text && (field.currentEditor() as? NSTextView)?.hasMarkedText() != true {
                field.stringValue = text
            }
            KeyboardNavigation.schedule(in: field.window)
        }
        final class Coordinator: NSObject, NSTextFieldDelegate {
            var owner: NativeField
            init(_ owner: NativeField) { self.owner = owner }
            func controlTextDidChange(_ notification: Notification) {
                guard let field = notification.object as? NSTextField else { return }
                owner.text = field.stringValue
            }
            func controlTextDidEndEditing(_ notification: Notification) {
                guard let field = notification.object as? NSTextField else { return }
                owner.text = field.stringValue
            }
        }
    }
}

final class AccessibleInputField: NSTextField {
    var defersTabNavigation = false
    override var acceptsFirstResponder: Bool { isEnabled && isEditable }
    override var canBecomeKeyView: Bool { acceptsFirstResponder && !isHiddenOrHasHiddenAncestor }
    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        KeyboardNavigation.schedule(in: window)
    }
}
final class AccessibleSecureField: NSSecureTextField {
    override var acceptsFirstResponder: Bool { isEnabled && isEditable }
    override var canBecomeKeyView: Bool { acceptsFirstResponder && !isHiddenOrHasHiddenAncestor }
    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        KeyboardNavigation.schedule(in: window)
    }
}
