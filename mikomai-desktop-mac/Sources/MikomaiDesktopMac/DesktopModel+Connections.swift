import Foundation
import MikomaiDesktopCore
extension DesktopModel {
    func saveConnection(_ connection: SavedConnection, password: String? = nil, enablePassword: String? = nil) {
        guard connection.validationError == nil else { return }
        do {
            try NativeCommands.updateCredentials(id: connection.id.uuidString, password: password, enablePassword: enablePassword)
            let updated = ConnectionCredentialPolicy.applying(password: password, enablePassword: enablePassword, to: connection)
            connections = ConnectionInventoryPolicy.saving(updated, into: connections)
            editingConnection = nil
        } catch { persistenceError = error.localizedDescription }
    }
    func deleteConnection(_ id: UUID) {
        do {
            try NativeCommands.updateCredentials(id: id.uuidString, password: "", enablePassword: "")
            connections = ConnectionInventoryPolicy.removing(id, from: connections)
        } catch { persistenceError = error.localizedDescription }
    }
}
