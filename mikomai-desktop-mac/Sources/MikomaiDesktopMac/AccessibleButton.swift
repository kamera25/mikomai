import AppKit
import SwiftUI

struct AccessibleControlAction {
    let title: String
    let perform: () -> Void
}

/// A single native accessibility element and key-view stop, including for
/// custom labels whose GeometryReader/animations have no accessible text.
struct AccessibleButton<Label: View>: NSViewRepresentable {
    let title: String
    let action: () -> Void
    var role: ButtonRole? = nil
    let label: Label
    var value: String = ""
    var toggleValue: Bool? = nil
    var accessibilityActions: [AccessibleControlAction] = []
    var onRename: (() -> Void)? = nil
    var fillsWidth = false
    @Environment(\.isEnabled) private var isEnabled
    @Environment(\.font) private var font
    @Environment(\.colorScheme) private var colorScheme
    @Environment(\.accessibleButtonAppearance) private var appearance
    @Environment(\.accessibleButtonKeyEquivalent) private var keyEquivalent
    @Environment(\.accessibleButtonHoverHighlight) private var hoverHighlight
    @Environment(\.accessibleButtonHoverCornerRadius) private var hoverCornerRadius

    init(_ title: String, value: String = "", role: ButtonRole? = nil, toggleValue: Bool? = nil, accessibilityActions: [AccessibleControlAction] = [], onRename: (() -> Void)? = nil, fillsWidth: Bool = false, action: @escaping () -> Void,
         @ViewBuilder label: () -> Label) {
        self.title = title
        self.value = value
        self.role = role
        self.toggleValue = toggleValue
        self.accessibilityActions = accessibilityActions
        self.onRename = onRename
        self.fillsWidth = fillsWidth
        self.action = action
        self.label = label()
    }

    func makeCoordinator() -> Coordinator { Coordinator(action: action) }

    func makeNSView(context: Context) -> KeyboardActionButton {
        let button = KeyboardActionButton()
        button.target = context.coordinator
        button.action = #selector(Coordinator.activate(_:))
        return button
    }

    func updateNSView(_ button: KeyboardActionButton, context: Context) {
        context.coordinator.action = action
        button.isEnabled = isEnabled
        button.appearanceKind = appearance
        button.showsHoverHighlight = hoverHighlight
        button.hoverCornerRadius = hoverCornerRadius
        button.onRename = onRename
        button.setAccessibilityCustomActions(accessibilityActions.map { item in
            NSAccessibilityCustomAction(name: item.title) { [weak button] in
                guard button?.isEnabled == true else { return false }
                item.perform()
                return true
            }
        })
        button.keyEquivalent = keyEquivalent
        button.keyEquivalentModifierMask = []
        button.setAccessibilityLabel(title)
        button.setAccessibilityRole(toggleValue == nil ? .button : .checkBox)
        if let toggleValue { button.setAccessibilityValue(NSNumber(value: toggleValue)) }
        else { button.setAccessibilityValue(value) }
        let content = AnyView(label
            .font(font)
            .environment(\.colorScheme, colorScheme)
            .foregroundStyle(role == .destructive ? Color.red : appearance == .prominent ? Color.white : Color.primary)
            .opacity(isEnabled ? 1 : 0.45)
            .disabled(!isEnabled).allowsHitTesting(false))
        if let hosting = button.labelView { hosting.rootView = content }
        else {
            let hosting = ButtonLabelHostingView(rootView: content)
            hosting.setAccessibilityElement(false)
            button.labelView = hosting
            button.addSubview(hosting)
        }
        button.invalidateIntrinsicContentSize()
        button.needsLayout = true
        KeyboardNavigation.schedule(in: button.window)
    }

    func sizeThatFits(_ proposal: ProposedViewSize, nsView: KeyboardActionButton, context: Context) -> CGSize? {
        let natural = nsView.intrinsicContentSize
        let width = fillsWidth ? (proposal.width ?? natural.width) : min(natural.width, proposal.width ?? natural.width)
        return CGSize(width: width, height: min(natural.height, proposal.height ?? natural.height))
    }

    @MainActor
    final class Coordinator: NSObject {
        var action: () -> Void
        init(action: @escaping () -> Void) { self.action = action }
        @objc func activate(_ sender: NSButton) {
            guard sender.isEnabled else { return }
            action()
        }
    }
}

extension AccessibleButton where Label == Text {
    init(_ title: String, role: ButtonRole? = nil, toggleValue: Bool? = nil, accessibilityActions: [AccessibleControlAction] = [], onRename: (() -> Void)? = nil, fillsWidth: Bool = false, action: @escaping () -> Void) {
        self.init(title, role: role, toggleValue: toggleValue, accessibilityActions: accessibilityActions, onRename: onRename, fillsWidth: fillsWidth, action: action) { Text(title) }
    }
}

enum AccessibleButtonAppearance { case standard, plain, prominent }
private struct AccessibleButtonAppearanceKey: EnvironmentKey {
    static let defaultValue = AccessibleButtonAppearance.standard
}
private struct AccessibleButtonKeyEquivalentKey: EnvironmentKey {
    static let defaultValue = ""
}
private struct AccessibleButtonHoverHighlightKey: EnvironmentKey {
    static let defaultValue = false
}
private struct AccessibleButtonHoverCornerRadiusKey: EnvironmentKey {
    static let defaultValue: CGFloat = 6
}
extension EnvironmentValues {
    var accessibleButtonKeyEquivalent: String {
        get { self[AccessibleButtonKeyEquivalentKey.self] }
        set { self[AccessibleButtonKeyEquivalentKey.self] = newValue }
    }
    var accessibleButtonAppearance: AccessibleButtonAppearance {
        get { self[AccessibleButtonAppearanceKey.self] }
        set { self[AccessibleButtonAppearanceKey.self] = newValue }
    }
    var accessibleButtonHoverHighlight: Bool {
        get { self[AccessibleButtonHoverHighlightKey.self] }
        set { self[AccessibleButtonHoverHighlightKey.self] = newValue }
    }
    var accessibleButtonHoverCornerRadius: CGFloat {
        get { self[AccessibleButtonHoverCornerRadiusKey.self] }
        set { self[AccessibleButtonHoverCornerRadiusKey.self] = newValue }
    }
}
extension View {
    func accessibleDefaultAction() -> some View {
        environment(\.accessibleButtonKeyEquivalent, "\r")
    }
    func accessibleCancelAction() -> some View {
        environment(\.accessibleButtonKeyEquivalent, "\u{1b}")
    }
    func accessibleButtonStyle(_ appearance: AccessibleButtonAppearance) -> some View {
        environment(\.accessibleButtonAppearance, appearance)
    }
    func accessibleButtonHoverHighlight(_ enabled: Bool = true, cornerRadius: CGFloat = 6) -> some View {
        environment(\.accessibleButtonHoverHighlight, enabled)
            .environment(\.accessibleButtonHoverCornerRadius, cornerRadius)
    }
}

final class ButtonLabelHostingView: NSHostingView<AnyView> {
    override func hitTest(_ point: NSPoint) -> NSView? { nil }
    override func accessibilityChildren() -> [Any]? { [] }
}

final class KeyboardActionButton: NSButton {
    var onRename: (() -> Void)?
    var labelView: ButtonLabelHostingView?
    var showsHoverHighlight = false {
        didSet {
            if showsHoverHighlight != oldValue {
                updateTrackingAreas()
                needsDisplay = true
            }
        }
    }
    var hoverCornerRadius: CGFloat = 6 {
        didSet {
            if hoverCornerRadius != oldValue {
                needsDisplay = true
            }
        }
    }
    private(set) var isHovered = false {
        didSet {
            if isHovered != oldValue {
                needsDisplay = true
            }
        }
    }
    private var trackingArea: NSTrackingArea?

    var appearanceKind = AccessibleButtonAppearance.standard {
        didSet {
            isBordered = appearanceKind != .plain
            bezelColor = appearanceKind == .prominent ? .controlAccentColor : nil
            needsDisplay = true
        }
    }

    init() {
        super.init(frame: .zero)
        title = ""
        setButtonType(.momentaryPushIn)
        bezelStyle = .rounded
        focusRingType = .exterior
        setAccessibilityElement(true)
        setAccessibilityRole(.button)
    }
    required init?(coder: NSCoder) { fatalError("init(coder:) is not supported") }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        if window == nil {
            isHovered = false
        }
        KeyboardNavigation.schedule(in: window)
    }
    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        if let trackingArea {
            removeTrackingArea(trackingArea)
            self.trackingArea = nil
        }
        guard showsHoverHighlight else {
            if isHovered { isHovered = false }
            return
        }
        let options: NSTrackingArea.Options = [.mouseEnteredAndExited, .activeInActiveApp, .inVisibleRect]
        let area = NSTrackingArea(rect: bounds, options: options, owner: self, userInfo: nil)
        addTrackingArea(area)
        self.trackingArea = area

        if let window, isEnabled {
            let mouseLoc = window.mouseLocationOutsideOfEventStream
            let locInView = convert(mouseLoc, from: nil)
            let inside = bounds.contains(locInView)
            if isHovered != inside {
                isHovered = inside
            }
        }
    }

    override func mouseEntered(with event: NSEvent) {
        super.mouseEntered(with: event)
        guard showsHoverHighlight, isEnabled else { return }
        isHovered = true
    }

    override func mouseExited(with event: NSEvent) {
        super.mouseExited(with: event)
        if isHovered {
            isHovered = false
        }
    }

    override func becomeFirstResponder() -> Bool {
        let result = super.becomeFirstResponder()
        if result { scrollToVisible(bounds) }
        return result
    }
    // All action buttons participate even if system keyboard navigation is off.
    override var acceptsFirstResponder: Bool { isEnabled }
    override var canBecomeKeyView: Bool { isEnabled && !isHiddenOrHasHiddenAncestor }
    override func accessibilityChildren() -> [Any]? { [] }
    override var intrinsicContentSize: NSSize {
        let size = labelView?.fittingSize ?? NSSize(width: 24, height: 24)
        let inset: CGFloat = appearanceKind == .plain ? 0 : 16
        return NSSize(width: max(24, size.width + inset), height: max(24, size.height + (inset == 0 ? 0 : 8)))
    }
    override func layout() {
        super.layout()
        let inset: CGFloat = appearanceKind == .plain ? 0 : 8
        labelView?.frame = bounds.insetBy(dx: inset, dy: appearanceKind == .plain ? 0 : 4)
    }
    override func draw(_ dirtyRect: NSRect) {
        super.draw(dirtyRect)
        if showsHoverHighlight && isHovered && isEnabled {
            let highlightColor = isHighlighted
                ? NSColor.textColor.withAlphaComponent(0.18)
                : NSColor.textColor.withAlphaComponent(0.10)
            highlightColor.setFill()
            let path = NSBezierPath(roundedRect: bounds, xRadius: hoverCornerRadius, yRadius: hoverCornerRadius)
            path.fill()
        }
    }
    override func drawFocusRingMask() {
        let radius = showsHoverHighlight ? hoverCornerRadius : 5
        NSBezierPath(roundedRect: bounds, xRadius: radius, yRadius: radius).fill()
    }
    override var focusRingMaskBounds: NSRect { bounds }

    override func performKeyEquivalent(with event: NSEvent) -> Bool {
        if [UInt16(36), 76].contains(event.keyCode) {
            // A focused action wins over a sheet's default Save action.
            if event.isARepeat { return true }
            if let focused = window?.firstResponder as? KeyboardActionButton, focused !== self { return false }
            if window?.firstResponder is KeyboardPopUpButton || window?.firstResponder is KeyboardSlider { return false }
            if let input = window?.firstResponder as? NSTextView, input.hasMarkedText() || !input.isFieldEditor { return false }
        }
        return super.performKeyEquivalent(with: event)
    }

    override func keyDown(with event: NSEvent) {
        let modifiers = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
        if isEnabled, window?.firstResponder === self, event.keyCode == 120,
           modifiers.intersection([.command, .control, .option, .shift]).isEmpty, let onRename {
            if !event.isARepeat { onRename() }
            return
        }
        // Consume Return/Space only while this enabled button owns keyboard
        // focus. Do not create a window-wide Return shortcut or override an IME.
        if isEnabled, window?.firstResponder === self,
           modifiers.intersection([.command, .control, .option, .shift]).isEmpty,
           [UInt16(36), 76, 49].contains(event.keyCode) {
            if !event.isARepeat { performClick(nil) }
            return
        }
        super.keyDown(with: event)
    }
}
