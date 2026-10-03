import AppKit
import SwiftUI

/// SwiftUI Forms can build a key-view loop containing only their text fields.
/// Link our native controls and editable fields together after layout, including
/// sheets and popovers, without changing a system-wide keyboard preference.
@MainActor
enum KeyboardNavigation {
    private static let managedWindows = NSHashTable<NSWindow>.weakObjects()
    private static var eventMonitor: Any?
    private static var pending = Set<ObjectIdentifier>()

    static func schedule(in window: NSWindow?) {
        guard let window else { return }
        let id = ObjectIdentifier(window)
        guard pending.insert(id).inserted else { return }
        DispatchQueue.main.async { [weak window] in
            pending.remove(id)
            guard let window else { return }
            rebuild(in: window)
        }
    }

    private static func controls(in window: NSWindow) -> [NSView] {
        guard let content = window.contentView else { return [] }
        func collect(_ view: NSView) -> [NSView] {
            guard !view.isHiddenOrHasHiddenAncestor else { return [] }
            let eligible: Bool
            switch view {
            case let button as KeyboardActionButton: eligible = button.isEnabled
            case let picker as KeyboardPopUpButton: eligible = picker.isEnabled
            case let slider as KeyboardSlider: eligible = slider.isEnabled
            case let field as NSTextField: eligible = field.isEnabled && field.isEditable
            case let text as NSTextView: eligible = text.isSelectable && !text.isFieldEditor
            case is NSTableView: eligible = true
            default: eligible = false
            }
            return (eligible ? [view] : []) + view.subviews.flatMap(collect)
        }
        return collect(content.superview ?? content)
    }

    static func rebuild(in window: NSWindow) {
        let controls = controls(in: window)
        guard !controls.isEmpty else { return }
        managedWindows.add(window)
        if eventMonitor == nil {
            eventMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { event in
                handleTab(event) ? nil : event
            }
        }
        window.autorecalculatesKeyViewLoop = false
        for (index, control) in controls.enumerated() {
            control.nextKeyView = controls[(index + 1) % controls.count]
        }
        window.initialFirstResponder = controls.first
    }

    /// Kept separate from the monitor so the real dispatch policy is testable.
    static func handleTab(_ event: NSEvent, in targetWindow: NSWindow? = nil) -> Bool {
        guard event.keyCode == 48,
              event.modifierFlags.intersection([.command, .control, .option]).isEmpty,
              let window = targetWindow ?? NSApp.keyWindow ?? event.window, managedWindows.contains(window) else { return false }
        // The composer owns suggestion acceptance and its input method. Other
        // field editors retain IME Tab while text is marked, too.
        if let text = window.firstResponder as? NSTextView {
            if text is ChatComposerTextView || text.hasMarkedText() { return false }
        }
        let backwards = event.modifierFlags.contains(.shift)
        if let text = window.firstResponder as? NSTextView, text.isFieldEditor {
            // Search fields can replace their results after the text event.
            // Let SwiftUI finish that update before focusing a result that may
            // otherwise be removed from the view hierarchy immediately.
            DispatchQueue.main.async { [weak window] in
                guard let window else { return }
                move(in: window, backwards: backwards)
            }
            return true
        }
        return move(in: window, backwards: backwards)
    }

    @discardableResult
    static func move(in window: NSWindow, backwards: Bool) -> Bool {
        guard managedWindows.contains(window) else { return false }
        let candidates = controls(in: window)
        guard !candidates.isEmpty else { return false }
        let responder = window.firstResponder
        let current = candidates.firstIndex { candidate in
            if candidate === responder { return true }
            if let field = candidate as? NSTextField, field.currentEditor() === responder { return true }
            if let focused = responder as? NSView { return focused.isDescendant(of: candidate) }
            return false
        }
        let direction = backwards ? -1 : 1
        let start = current ?? (backwards ? 0 : candidates.count - 1)
        for offset in 1...candidates.count {
            let index = (start + direction * offset + candidates.count) % candidates.count
            if window.makeFirstResponder(candidates[index]) {
                candidates[index].scrollToVisible(candidates[index].bounds)
                return true
            }
        }
        return false
    }
}

struct KeyboardNavigationScope: NSViewRepresentable {
    final class Observer: NSView {
        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            KeyboardNavigation.schedule(in: window)
        }
        override func layout() {
            super.layout()
            KeyboardNavigation.schedule(in: window)
        }
    }
    func makeNSView(context: Context) -> Observer { Observer() }
    func updateNSView(_ view: Observer, context: Context) { KeyboardNavigation.schedule(in: view.window) }
}
