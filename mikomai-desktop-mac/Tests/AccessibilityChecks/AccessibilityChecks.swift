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

    // Display-only content must be in the same real Tab dispatch loop as actions.
    var agentMessage = ChatMessage(role: .assistant, text: "回答の先頭\n\n回答の最後")
    agentMessage.agentGoal = "経路を確認する"
    agentMessage.agentProgress = [AgentProgressEntry(phase: "実行", nextAction: "経路を取得", detail: "省略せず読み取る実行詳細")]
    let conversation = ChatSession(title: "本文フォーカス検証", messages: [
        ChatMessage(role: .user, text: "ユーザーの質問全文", attachments: ["config.txt"]), agentMessage
    ])
    model.sessions = [conversation]
    model.activeSessionID = conversation.id
    window.contentView = host
    pump()
    func readableViews(_ root: NSView) -> [ReadableContentView] {
        descendants(root).compactMap { $0 as? ReadableContentView }
    }
    let transcript = readableViews(host)
    for prefix in ["ユーザーの発言:", "AIの発言:", "エージェントの状態:", "エージェントの目的:", "エージェントの次のアクション:", "エージェントの詳細:"] {
        precondition(transcript.contains { ($0.accessibilityLabel() ?? "").hasPrefix(prefix) }, "Missing readable content: \(prefix)")
    }
    let answer = transcript.first { ($0.accessibilityLabel() ?? "").hasPrefix("AIの発言:") }!
    precondition((answer.accessibilityValue() as? String)?.contains("回答の最後") == true)
    let user = transcript.first { ($0.accessibilityLabel() ?? "").hasPrefix("ユーザーの発言:") }!
    precondition((user.accessibilityValue() as? String)?.contains("config.txt") == true)
    KeyboardNavigation.rebuild(in: window)
    window.makeFirstResponder(button("チャット", in: host))
    var contentVisited = Set<ObjectIdentifier>()
    for _ in 0..<60 {
        if !KeyboardNavigation.handleTab(key(48), in: window) {
            guard let composer = window.firstResponder as? ChatComposerTextView else { fatalError("Unexpected Tab owner") }
            composer.keyDown(with: key(48))
        }
        pump()
        if let content = window.firstResponder as? ReadableContentView { contentVisited.insert(ObjectIdentifier(content)) }
    }
    precondition(transcript.allSatisfy { contentVisited.contains(ObjectIdentifier($0)) }, "Tab reaches user, AI and every agent field")
    window.makeFirstResponder(answer)
    precondition(KeyboardNavigation.handleTab(key(48, modifiers: .shift), in: window))
    precondition(window.firstResponder !== answer)
    let disclosure = buttons(host).first { ($0.accessibilityLabel() ?? "").hasPrefix("実行内容") }!
    window.makeFirstResponder(disclosure)
    disclosure.keyDown(with: key(36))
    pump()
    let detail = readableViews(host).first { ($0.accessibilityLabel() ?? "").hasPrefix("エージェントの実行内容 1:") }!
    precondition(window.makeFirstResponder(detail) && detail.canBecomeKeyView)
    precondition((detail.accessibilityValue() as? String)?.contains("経路を取得") == true)
    print("PASS: real chat Tab/Shift+Tab reaches full user/AI content and agent fields; expanded execution is readable")

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
    let fieldNames = ["名前（必須）", "ホスト名またはIPアドレス（必須）", "ポート", "ユーザー名", "パスワード", "Enable パスワード"]
    precondition(fields.count == fieldNames.count)
    for name in fieldNames {
        let field = fields.first { $0.accessibilityLabel() == name }!
        precondition(!(field.accessibilityHelp() ?? "").isEmpty, "Input help persists for \(name)")
        precondition(field.canBecomeKeyView)
    }
    precondition(fields.filter { $0 is NSSecureTextField }.count == 2)
    precondition(fields.first { $0.accessibilityLabel() == "名前（必須）" }?.stringValue == "検証ホスト")
    window.makeFirstResponder(fields[0])
    precondition(KeyboardNavigation.handleTab(key(48), in: window))
    precondition(fields[1].currentEditor() === window.firstResponder, "Native field Tab changes focus synchronously before the next typed character")
    window.makeFirstResponder(fields[0])
    var visited = Set<ObjectIdentifier>()
    var visitedFields = Set<String>()
    for _ in 0..<30 {
        precondition(KeyboardNavigation.handleTab(key(48), in: window))
        pump()
        if let responder = window.firstResponder { visited.insert(ObjectIdentifier(responder)) }
        for field in fields where field.currentEditor() === window.firstResponder || field === window.firstResponder {
            visitedFields.insert(field.accessibilityLabel() ?? "")
        }
    }
    precondition(Set(fieldNames).isSubset(of: visitedFields), "Tab reaches every named input, including protected fields")
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

    let invalidEditor = NSHostingView(rootView: ConnectionEditor(connection: SavedConnection(name: "", host: "")) { _, _, _ in saves += 1 })
    window.contentView = invalidEditor
    invalidEditor.layoutSubtreeIfNeeded()
    pump()
    let error = readableViews(invalidEditor).first { ($0.accessibilityLabel() ?? "").hasPrefix("入力エラー:") }!
    let privacy = readableViews(invalidEditor).first { ($0.accessibilityLabel() ?? "").hasPrefix("資格情報の保存について:") }!
    precondition(window.makeFirstResponder(error) && window.makeFirstResponder(privacy))
    precondition(!button("保存", in: invalidEditor).isEnabled)
    let invalidName = descendants(invalidEditor).compactMap { $0 as? AccessibleInputField }.first { $0.accessibilityLabel() == "名前（必須）" }!
    precondition((invalidName.accessibilityHelp() ?? "").contains("入力エラー:"))
    print("PASS: registration has six persistent named/helped fields, protected passwords, readable validation and privacy text")

    // Native edit notifications round-trip the binding without losing the AX
    // name; protected values must never become public accessibility strings.
    var input = "before"
    var secret = "test-only-password"
    let fieldHost = NSHostingView(rootView: VStack {
        AccessibleTextField(title: "入力検証", text: Binding(get: { input }, set: { input = $0 }), help: "説明")
        AccessibleTextField(title: "保護入力検証", text: Binding(get: { secret }, set: { secret = $0 }), isSecure: true)
    })
    window.contentView = fieldHost
    fieldHost.layoutSubtreeIfNeeded()
    pump()
    let nativeInput = descendants(fieldHost).compactMap { $0 as? AccessibleInputField }.first!
    nativeInput.stringValue = "after"
    nativeInput.delegate?.controlTextDidChange?(Notification(name: NSControl.textDidChangeNotification, object: nativeInput))
    pump()
    precondition(input == "after" && nativeInput.accessibilityLabel() == "入力検証")
    let protected = descendants(fieldHost).compactMap { $0 as? AccessibleSecureField }.first!
    precondition(!(String(describing: protected.accessibilityValue())).contains(secret))
    precondition(!(protected.accessibilityLabel() ?? "").contains(secret))
    print("PASS: input edits update binding while names persist; protected input does not expose its value")

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
    let cells = readableViews(host)
    for column in ["名前", "ホスト", "ポート", "ユーザー", "機器タイプ", "資格情報"] {
        precondition(cells.contains { ($0.accessibilityLabel() ?? "").hasPrefix("検証機器の\(column):") }, "Missing readable table column: \(column)")
    }
    precondition(cells.first { ($0.accessibilityLabel() ?? "").hasPrefix("検証機器のユーザー:") }?.accessibilityValue() as? String == "未設定")
    KeyboardNavigation.rebuild(in: window)
    window.makeFirstResponder(button("機器を追加", in: host))
    var cellsVisited = Set<String>()
    for _ in 0..<40 {
        precondition(KeyboardNavigation.handleTab(key(48), in: window))
        pump()
        if let cell = window.firstResponder as? ReadableContentView { cellsVisited.insert(cell.accessibilityLabel() ?? "") }
    }
    precondition(cells.allSatisfy { cellsVisited.contains($0.accessibilityLabel() ?? "") }, "Every table cell is reachable through Tab; visited: \(cellsVisited.sorted()); cells: \(cells.map { "\($0.accessibilityLabel() ?? "") enabled=\($0.isEnabled) hidden=\($0.isHiddenOrHasHiddenAncestor) key=\($0.canBecomeKeyView) frame=\($0.frame)" })")
    print("PASS: host table Tab reaches all six columns, identifies row/column/value, and names empty cells")
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
