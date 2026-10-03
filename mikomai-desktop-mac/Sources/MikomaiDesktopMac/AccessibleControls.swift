import AppKit
import SwiftUI

struct AccessiblePicker<Value: Hashable>: View {
    let title: String
    @Binding var selection: Value
    let options: [(Value, String)]
    var showsTitle = true

    init(_ title: String, selection: Binding<Value>, options: [(Value, String)], showsTitle: Bool = true) {
        self.title = title
        self._selection = selection
        self.options = options
        self.showsTitle = showsTitle
    }

    var body: some View {
        HStack {
            if showsTitle { Text(title) }
            NativePicker(title: title, selection: $selection, options: options)
        }
    }

    private struct NativePicker: NSViewRepresentable {
        let title: String
        @Binding var selection: Value
        let options: [(Value, String)]
        @Environment(\.isEnabled) private var isEnabled

        func makeCoordinator() -> Coordinator { Coordinator(self) }
        func makeNSView(context: Context) -> KeyboardPopUpButton {
            let control = KeyboardPopUpButton(frame: .zero, pullsDown: false)
            control.target = context.coordinator
            control.action = #selector(Coordinator.choose(_:))
            return control
        }
        func updateNSView(_ control: KeyboardPopUpButton, context: Context) {
            context.coordinator.owner = self
            let titles = options.map(\.1)
            if control.itemTitles != titles {
                control.removeAllItems()
                control.addItems(withTitles: titles)
            }
            if let index = options.firstIndex(where: { $0.0 == selection }) { control.selectItem(at: index) }
            else { control.select(nil) }
            control.isEnabled = isEnabled
            control.setAccessibilityLabel(title)
            KeyboardNavigation.schedule(in: control.window)
        }
        final class Coordinator: NSObject {
            var owner: NativePicker
            init(_ owner: NativePicker) { self.owner = owner }
            @objc func choose(_ sender: NSPopUpButton) {
                guard sender.isEnabled, owner.options.indices.contains(sender.indexOfSelectedItem) else { return }
                owner.selection = owner.options[sender.indexOfSelectedItem].0
            }
        }
    }
}

final class KeyboardPopUpButton: NSPopUpButton {
    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        KeyboardNavigation.schedule(in: window)
    }
    override var acceptsFirstResponder: Bool { isEnabled }
    override var canBecomeKeyView: Bool { isEnabled && !isHiddenOrHasHiddenAncestor }
    override func keyDown(with event: NSEvent) {
        if isEnabled, window?.firstResponder === self,
           event.modifierFlags.intersection([.command, .option, .control, .shift]).isEmpty,
           [UInt16(36), 76, 49].contains(event.keyCode) {
            if !event.isARepeat { performClick(nil) }
            return
        }
        super.keyDown(with: event)
    }
}

struct AccessibleToggle<Label: View>: View {
    let title: String
    @Binding var isOn: Bool
    let label: Label

    init(_ title: String, isOn: Binding<Bool>, @ViewBuilder label: () -> Label) {
        self.title = title
        self._isOn = isOn
        self.label = label()
    }
    var body: some View {
        AccessibleButton(title, toggleValue: isOn, fillsWidth: true, action: { isOn.toggle() }) {
            HStack(spacing: 16) {
                label.frame(maxWidth: .infinity, alignment: .leading)
                ZStack(alignment: isOn ? .trailing : .leading) {
                    Capsule().fill(isOn ? Color.accentColor : Color.secondary.opacity(0.4))
                    Circle().fill(Color.white).padding(2)
                        .frame(width: 20, height: 20)
                }
                .frame(width: 34, height: 20)
                .accessibilityHidden(true)
            }
            .frame(minHeight: 28)
            .contentShape(Rectangle())
        }
        .accessibleButtonStyle(.plain)
    }
}
extension AccessibleToggle where Label == Text {
    init(_ title: String, isOn: Binding<Bool>) {
        self.init(title, isOn: isOn) { Text(title) }
    }
}

struct AccessibleDisclosureGroup<Content: View>: View {
    let title: String
    let content: Content
    @State private var isExpanded = false
    init(_ title: String, @ViewBuilder content: () -> Content) {
        self.title = title
        self.content = content()
    }
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            AccessibleButton(title, value: isExpanded ? "展開中" : "折りたたみ中", fillsWidth: true, action: { isExpanded.toggle() }) {
                HStack {
                    Image(systemName: isExpanded ? "chevron.down" : "chevron.right")
                    Text(title)
                    Spacer(minLength: 0)
                }.frame(minHeight: 28).contentShape(Rectangle())
            }.accessibleButtonStyle(.plain)
            if isExpanded { content }
        }
    }
}

struct AccessibleSlider: NSViewRepresentable {
    let title: String
    @Binding var value: Double
    let range: ClosedRange<Double>
    let step: Double
    @Environment(\.isEnabled) private var isEnabled
    init(_ title: String, value: Binding<Double>, in range: ClosedRange<Double>, step: Double) {
        self.title = title
        self._value = value
        self.range = range
        self.step = step
    }
    func makeCoordinator() -> Coordinator { Coordinator(self) }
    func makeNSView(context: Context) -> KeyboardSlider {
        let control = KeyboardSlider()
        control.target = context.coordinator
        control.action = #selector(Coordinator.change(_:))
        return control
    }
    func updateNSView(_ control: KeyboardSlider, context: Context) {
        context.coordinator.owner = self
        control.minValue = range.lowerBound
        control.maxValue = range.upperBound
        control.doubleValue = value
        control.step = step
        control.isEnabled = isEnabled
        control.setAccessibilityLabel(title)
        KeyboardNavigation.schedule(in: control.window)
    }
    final class Coordinator: NSObject {
        var owner: AccessibleSlider
        init(_ owner: AccessibleSlider) { self.owner = owner }
        @objc func change(_ sender: NSSlider) {
            guard sender.isEnabled else { return }
            let ticks = ((sender.doubleValue - owner.range.lowerBound) / owner.step).rounded()
            owner.value = min(owner.range.upperBound, max(owner.range.lowerBound, owner.range.lowerBound + ticks * owner.step))
        }
    }
}
final class KeyboardSlider: NSSlider {
    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        KeyboardNavigation.schedule(in: window)
    }
    var step: Double = 1
    override var acceptsFirstResponder: Bool { isEnabled }
    override var canBecomeKeyView: Bool { isEnabled && !isHiddenOrHasHiddenAncestor }
    private func adjust(_ amount: Double) -> Bool {
        guard isEnabled else { return false }
        doubleValue = min(maxValue, max(minValue, doubleValue + amount))
        sendAction(action, to: target)
        return true
    }
    override func accessibilityPerformIncrement() -> Bool { adjust(step) }
    override func accessibilityPerformDecrement() -> Bool { adjust(-step) }
    override func keyDown(with event: NSEvent) {
        if isEnabled, window?.firstResponder === self,
           event.modifierFlags.intersection([.command, .control, .option]).isEmpty {
            switch event.keyCode {
            case 123, 125: _ = adjust(-step); return
            case 124, 126: _ = adjust(step); return
            default: break
            }
        }
        super.keyDown(with: event)
    }
}
