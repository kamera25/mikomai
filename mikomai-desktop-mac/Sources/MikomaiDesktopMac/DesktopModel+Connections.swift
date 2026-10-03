import Foundation
import MikomaiDesktopCore

extension DesktopModel {
    // MARK: - Connections & Keychain Management

    func saveConnection(_ connection: SavedConnection, password: String? = nil, enablePassword: String? = nil) {
        guard connection.validationError == nil else { return }
        var updated = connection
        credentialPersistence.save(for: connection.id, password: password, enablePassword: enablePassword)
        updated = ConnectionCredentialPolicy.applying(
            password: password,
            enablePassword: enablePassword,
            to: updated
        )

        connections = ConnectionInventoryPolicy.saving(updated, into: connections)
        editingConnection = nil
    }

    func deleteConnection(_ id: UUID) {
        connections = ConnectionInventoryPolicy.removing(id, from: connections)
        credentialPersistence.delete(for: id)
    }

    func importLegacyDevices(fromJSON data: Data) throws -> LegacyConnectionImportResult {
        let result = try LegacyConnectionImporter.importJSON(data, existing: connections)
        connections.append(contentsOf: result.imported)
        return result
    }
}
