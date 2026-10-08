import MikomaiDesktopCore
import ObjectiveC

// Supply the clicked row normally set by AppKit mouse tracking. This permits
// exercising SwiftUI's real Table double-action coordinator without a key window.
@MainActor private final class DoubleClickContext: NSObject {
    static var row = 0
    @objc func clickedRowForCheck() -> Int { Self.row }
}

@main struct ConnectionInteractionChecks {
    @MainActor static func main() {
        let dataRoot = FileManager.default.temporaryDirectory.appendingPathComponent("mikomai-connections-check-\(UUID().uuidString)")
        setenv("MIKOMAI_DATA_DIR", dataRoot.path, 1)
        setenv("MIKOMAI_GRAPH_DB_PATH", dataRoot.appendingPathComponent("graph").path, 1)
        defer { try? FileManager.default.removeItem(at: dataRoot) }
        _ = NSApplication.shared
        let model = DesktopModel()
        model.connections = [SavedConnection(name: "検証機器", host: "192.0.2.1")]
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 1200, height: 800),
                              styleMask: [.titled], backing: .buffered, defer: false)
        func descendants(_ view: NSView) -> [NSView] { [view] + view.subviews.flatMap(descendants) }
        func pump() {
            RunLoop.main.run(until: Date().addingTimeInterval(0.15))
            window.contentView?.layoutSubtreeIfNeeded()
        }
        model.connections.append(SavedConnection(name: "別の検証機器", host: "192.0.2.2"))
        let inventory = NSHostingView(rootView: ConnectionsWorkspace(model: model))
        window.contentView = inventory
        pump()
        let table = descendants(inventory).compactMap { $0 as? NSTableView }.first!
        precondition(table.numberOfRows == 2)
        precondition(table.rect(ofRow: 0).height >= 36)
        precondition(table.rect(ofRow: 0).width == table.bounds.width, "Native rows span the entire table")
        let clickedRowMethod = class_getInstanceMethod(NSTableView.self, #selector(getter: NSTableView.clickedRow))!
        let contextMethod = class_getInstanceMethod(DoubleClickContext.self, #selector(DoubleClickContext.clickedRowForCheck))!
        let originalImplementation = method_setImplementation(clickedRowMethod, method_getImplementation(contextMethod))
        defer { method_setImplementation(clickedRowMethod, originalImplementation) }
        for row in 0..<2 {
            model.editingConnection = nil
            table.selectRowIndexes(IndexSet(integer: row), byExtendingSelection: false)
            pump()
            precondition(model.editingConnection == nil, "Selecting a row must not open the editor")
            DoubleClickContext.row = row
            guard let doubleAction = table.doubleAction else {
                fatalError("Device table must expose its native double-click action")
            }
            precondition(NSApplication.shared.sendAction(doubleAction, to: table.target, from: table))
            pump()
            precondition(model.editingConnection?.id == model.connections[row].id, "Native double-click action opens the selected device, including after changing rows")
        }
        print("PASS: full-width rows are at least 36pt; selection does not edit; native double-click action opens each selected device")
        model.editingConnection = nil
        let edit = descendants(inventory).compactMap { $0 as? KeyboardActionButton }
            .first { $0.accessibilityLabel() == "機器を編集: 検証機器" }!
        edit.performClick(nil)
        precondition(model.editingConnection?.id == model.connections[0].id, "Existing edit button still opens its device")
        precondition(descendants(inventory).compactMap { $0 as? KeyboardActionButton }
            .contains { $0.accessibilityLabel() == "機器を削除: 検証機器" })
        print("PASS: existing edit button and separate delete control remain available")
        for (password, enable) in [(false, false), (true, false), (false, true), (true, true)] {
            model.connections[0].hasPassword = password
            model.connections[0].hasEnablePassword = enable
            pump()
            let cells = descendants(inventory).compactMap { $0 as? ReadableContentView }
            for (column, registered) in [("パスワード", password), ("Enableパスワード", enable)] {
                let cell = cells.first { ($0.accessibilityLabel() ?? "").hasPrefix("検証機器の\(column):") }!
                precondition(cell.accessibilityValue() as? String == (registered ? "*****" : "未設定"),
                             "Each credential must independently show a fixed mask only when registered")
            }
        }
        print("PASS: password and Enable columns independently show ***** or 未設定 for all four registration states")
        for (password, enable) in [(false, false), (true, false), (false, true), (true, true)] {
            var saved = false
            var receivedPassword: String?
            var receivedEnable: String?
            let connection = SavedConnection(name: "編集検証", host: "192.0.2.3", hasPassword: password, hasEnablePassword: enable)
            let editor = NSHostingView(rootView: ConnectionEditor(connection: connection) { _, password, enable in
                saved = true
                receivedPassword = password
                receivedEnable = enable
            })
            window.contentView = editor
            pump()
            let fields = descendants(editor).compactMap { $0 as? AccessibleSecureField }
            precondition(fields.count == 2)
            for (title, registered) in [("パスワード", password), ("Enable パスワード", enable)] {
                let field = fields.first { $0.accessibilityLabel() == title }!
                precondition(field.placeholderString == (registered ? "*****" : ""))
                precondition(field.stringValue.isEmpty, "Mask must be a placeholder, never a credential value")
                precondition((field.accessibilityHelp() ?? "").contains(registered ? "登録済み" : "未登録"))
            }
            let save = descendants(editor).compactMap { $0 as? KeyboardActionButton }.first { $0.accessibilityLabel() == "保存" }!
            save.performClick(nil)
            precondition(saved && receivedPassword == nil && receivedEnable == nil, "Unchanged masks must not overwrite stored credentials")
        }
        print("PASS: editor placeholders reflect all registration states; unchanged Save preserves both credentials")
    }
}
