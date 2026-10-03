import MikomaiDesktopCore

@main struct AccessibilityChecks {
 @MainActor static func main() {
    setenv("MIKOMAI_SETTINGS_PATH", "/private/tmp/mikomai-full-check/settings.json", 1)
    _ = NSApplication.shared
    let suite = "mikomai.accessibility-check.\(UUID().uuidString)"
    let defaults = UserDefaults(suiteName: suite)!
    defer { defaults.removePersistentDomain(forName: suite) }
    let dataRoot = FileManager.default.temporaryDirectory.appendingPathComponent(suite)
    setenv("MIKOMAI_DATA_DIR", dataRoot.path, 1)
    setenv("MIKOMAI_GRAPH_DB_PATH", dataRoot.appendingPathComponent("graph").path, 1)
    defer { try? FileManager.default.removeItem(at: dataRoot) }
    let model = DesktopModel(defaults: defaults)
    let host = NSHostingView(rootView: DesktopWindow(model: model))
    let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 1200, height: 800),
                          styleMask: [.titled], backing: .buffered, defer: false)
    window.contentView = host
    window.autorecalculatesKeyViewLoop = true
    func pump() { RunLoop.main.run(until: Date().addingTimeInterval(0.15)); host.layoutSubtreeIfNeeded() }
    func descendants(_ view: NSView) -> [NSView] { [view] + view.subviews.flatMap(descendants) }
    func buttons(_ root: NSView) -> [KeyboardActionButton] { descendants(root).compactMap { $0 as? KeyboardActionButton } }
    func button(_ title: String, in root: NSView) -> KeyboardActionButton {
        guard let result = buttons(root).first(where: { $0.accessibilityLabel() == title }) else {
            fatalError("Missing accessible button: \(title); found \(buttons(root).map { $0.accessibilityLabel() ?? "" })")
        }
        return result
    }
    func key(_ code: UInt16, modifiers: NSEvent.ModifierFlags = [], repeatKey: Bool = false) -> NSEvent {
        NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: modifiers, timestamp: 0,
                         windowNumber: window.windowNumber, context: nil,
                         characters: code == 48 ? "\t" : code == 49 ? " " : "\r",
                         charactersIgnoringModifiers: code == 48 ? "\t" : code == 49 ? " " : "\r",
                         isARepeat: repeatKey, keyCode: code)!
    }
    pump()
    // Production history must expose the whole title independently of the
    // GeometryReader, truncation, animation and keyboard navigation setting.
    model.createSession()
    let secondID = model.activeSessionID!
    model.renameSession(secondID, title: "選択と読み上げの検証")
    model.createSession()
    pump()
    let row = button("選択と読み上げの検証", in: host)
    precondition(row.accessibilityRole() == .button && row.accessibilityValue() as? String == "未選択")
    precondition(row.accessibilityChildren()?.isEmpty == true, "History is one named button, not an unnamed container")
    precondition(row.canBecomeKeyView && window.makeFirstResponder(row))
    row.keyDown(with: key(36))
    pump()
    precondition(model.activeSessionID == secondID, "Return selects a production history row")
    precondition(button("選択と読み上げの検証", in: host).accessibilityValue() as? String == "選択中")
    let namedActions = row.accessibilityCustomActions() ?? []
    precondition(namedActions.map(\.name) == ["名前を変更", "削除"])
    precondition(window.makeFirstResponder(row))
    row.keyDown(with: key(120, modifiers: [.function, .numericPad]))
    pump()
    precondition(descendants(host).contains { $0 is ChatTitleEditor.TitleField }, "F2 opens the native rename field")
    print("PASS: production history label, single accessibility element, selected value, Return selection and F2 rename")

    // Every mounted action has a name and one focus stop. Audit each workspace.
    for workspace in Workspace.allCases {
        model.workspace = workspace
        pump()
        let controls = buttons(host)
        precondition(!controls.isEmpty)
        for control in controls {
            precondition(!(control.accessibilityLabel() ?? "").isEmpty, "No unnamed action buttons")
            if control.isEnabled && !control.isHiddenOrHasHiddenAncestor {
                precondition(control.canBecomeKeyView && control.acceptsFirstResponder)
            }
        }
        print("PASS: \(workspace.rawValue) named action controls: \(controls.count)")
    }

    for category in ["チャット・通信", "LLM モデル", "Vision (画像)", "ナレッジ RAG", "設定"] {
        let control = button(category, in: host)
        window.makeFirstResponder(control)
        control.keyDown(with: key(36))
        pump()
        for action in buttons(host) { precondition(!(action.accessibilityLabel() ?? "").isEmpty) }
        print("PASS: settings category \(category) keyboard selection and accessible actions")
    }

    // Host editor uses a save spy; no real connection or credentials are saved.
    var saves = 0
    let editorHost = NSHostingView(rootView: ConnectionEditor(connection: SavedConnection(name: "検証ホスト", host: "192.0.2.1")) { _, _, _ in saves += 1 })
    window.contentView = editorHost
    editorHost.layoutSubtreeIfNeeded()
    RunLoop.main.run(until: Date().addingTimeInterval(0.2))
    let deviceType = button("機器タイプを選択", in: editorHost)
    let cancel = button("キャンセル", in: editorHost)
    let save = button("保存", in: editorHost)
    let picker = descendants(editorHost).compactMap { $0 as? KeyboardPopUpButton }.first!
    precondition(picker.accessibilityLabel() == "接続方式" && picker.canBecomeKeyView)
    for control in [deviceType, cancel, save] { precondition(control.canBecomeKeyView && !control.isHiddenOrHasHiddenAncestor) }
    KeyboardNavigation.rebuild(in: window)
    let fields = descendants(editorHost).compactMap { $0 as? NSTextField }.filter { $0.isEditable }
    precondition(!fields.isEmpty)
    window.makeFirstResponder(fields[0])
    var visited = Set<ObjectIdentifier>()
    for _ in 0..<30 {
        precondition(KeyboardNavigation.handleTab(key(48), in: window))
        pump()
        if let responder = window.firstResponder { visited.insert(ObjectIdentifier(responder)) }
    }
    for control in [deviceType, cancel, save] { precondition(visited.contains(ObjectIdentifier(control)), "Tab loop reaches \(control.accessibilityLabel()!)") }
    window.makeFirstResponder(save)
    precondition(KeyboardNavigation.handleTab(key(48, modifiers: .shift), in: window))
    precondition(window.firstResponder === cancel, "Shift+Tab returns from Save to Cancel")
    window.makeFirstResponder(picker)
    precondition(!save.performKeyEquivalent(with: key(36)), "Picker Return opens the choices, not default Save")
    window.makeFirstResponder(cancel)
    precondition(!save.performKeyEquivalent(with: key(36)), "Focused Cancel cannot invoke default Save")
    precondition(saves == 0)
    window.makeFirstResponder(save)
    save.keyDown(with: key(36))
    precondition(saves == 1, "Return on Save executes exactly once")
    save.keyDown(with: key(36, repeatKey: true))
    precondition(saves == 1, "Key repeat cannot submit twice")
    print("PASS: host editor Tab reaches type/Cancel/Save; focused action beats default Save; Return saves once")

    // Disabled controls cannot be focused or invoked through AX or the keyboard.
    let disabledHost = NSHostingView(rootView: AccessibleButton("無効", action: { saves += 1 }).disabled(true))
    window.contentView = disabledHost
    disabledHost.layoutSubtreeIfNeeded()
    let disabled = button("無効", in: disabledHost)
    precondition(!disabled.canBecomeKeyView && !disabled.acceptsFirstResponder)
    disabled.keyDown(with: key(36))
    _ = disabled.accessibilityPerformPress()
    precondition(saves == 1)

    var enabled = false
    let toggleHost = NSHostingView(rootView: AccessibleToggle("切替", isOn: Binding(get: { enabled }, set: { enabled = $0 })))
    window.contentView = toggleHost
    toggleHost.layoutSubtreeIfNeeded()
    let toggle = button("切替", in: toggleHost)
    precondition(toggle.accessibilityRole() == .checkBox)
    window.makeFirstResponder(toggle)
    toggle.keyDown(with: key(49))
    precondition(enabled, "Space toggles an accessible switch")

    var sliderValue = 10.0
    let sliderHost = NSHostingView(rootView: AccessibleSlider("保持数", value: Binding(get: { sliderValue }, set: { sliderValue = $0 }), in: 0...20, step: 1))
    window.contentView = sliderHost
    sliderHost.layoutSubtreeIfNeeded()
    let slider = descendants(sliderHost).compactMap { $0 as? KeyboardSlider }.first!
    window.makeFirstResponder(slider)
    slider.keyDown(with: key(124))
    precondition(sliderValue == 11, "Right arrow adjusts by the actual integer step")
    precondition(slider.accessibilityPerformDecrement() && sliderValue == 10)
    model.workspace = .connections
    model.connections = [SavedConnection(name: "検証機器", host: "192.0.2.1")]
    window.contentView = host
    pump()
    let edit = button("機器を編集: 検証機器", in: host)
    window.makeFirstResponder(edit)
    edit.keyDown(with: key(36))
    pump()
    precondition(model.editingConnection?.name == "検証機器", "Table edit action can be focused and activated independently")
    precondition(button("機器を削除: 検証機器", in: host).accessibilityRole() == .button)
    print("PASS: table edit/delete remain separately named focusable actions")
    print("PASS: disabled action guards, Space switch activation, slider keyboard/AX step")
 }
}
