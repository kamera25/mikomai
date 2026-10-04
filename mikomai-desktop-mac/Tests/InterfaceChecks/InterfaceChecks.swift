import MikomaiDesktopCore
import MikomaiFFI

// Explicit opt-in read against a saved target. Isolated graph/events, no settings writes.
@main struct InterfaceCheck {
    static func main() {
        let root = URL(fileURLWithPath: ProcessInfo.processInfo.environment["MIKOMAI_EXECUTION_CHECK_DIR"]!)
        setenv("MIKOMAI_DATA_DIR", root.appendingPathComponent("data").path, 1)
        setenv("MIKOMAI_GRAPH_DB_PATH", root.appendingPathComponent("graph").path, 1)
        let target = ProcessInfo.processInfo.environment["MIKOMAI_INTERFACE_CHECK_TARGET"]!
        let interface = ProcessInfo.processInfo.environment["MIKOMAI_INTERFACE_CHECK_LAN"] ?? "LAN1"
        let defaults = UserDefaults(suiteName: "com.mikomai.desktop.mac")!
        guard let data = defaults.data(forKey: "mikomai.desktop.mac.connections.v1"),
              let connections = try? JSONDecoder().decode([SavedConnection].self, from: data),
              let connection = connections.first(where: { $0.name == target }) else {
            fatalError("saved target not found")
        }
        let credentialPersistence = ConnectionCredentialPersistence(store: KeychainCredentialAdapter())
        let prompt = ProcessInfo.processInfo.environment["MIKOMAI_INTERFACE_CHECK_PROMPT"] ?? "\(target)の\(interface)がupしているか確認して"
        if let model = ProcessInfo.processInfo.environment["MIKOMAI_INTERFACE_CHECK_MODEL"] {
            let loaded = model.withCString { mikomai_model_load($0) }
            let status = loaded.status
            mikomai_result_free(loaded)
            precondition(status == 0, "model load failed")
        }
        let answer = DesktopModel.askRustStreaming(prompt, history: "", documents: "", knowledge: root.path,
            attachments: "", connections: [connection], credentialPersistence: credentialPersistence,
            onOperationPlan: { _ in fatalError("read-only check must not propose changes") },
            onDebug: { print($0); fflush(stdout) }, onToolResult: { _ in }, onChunk: { _, _ in })
        precondition(!answer.contains("エラー:"), answer)
        precondition(answer.contains("upです") || answer.contains("downです") || answer.contains("確認不能") || answer.contains("確認できませんでした"), answer)
        print(answer)
    }
}
