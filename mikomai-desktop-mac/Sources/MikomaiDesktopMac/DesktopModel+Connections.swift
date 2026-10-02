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
        connectionStatuses.removeValue(forKey: id)
        credentialPersistence.delete(for: id)
    }

    func testConnection(_ connection: SavedConnection) {
        let host = connection.host
        let port = UInt16(connection.effectivePort) ?? 22
        let id = connection.id

        Task.detached(priority: .userInitiated) {
            let res = Self.testTCP(host: host, port: port, timeoutMs: 2500)
            await MainActor.run {
                self.connectionStatuses[id] = ConnectionTestStatus(
                    success: res.success,
                    message: res.message,
                    latencyMs: res.latencyMs,
                    timestamp: Date()
                )
            }
        }
    }

    func testTcpDirect() {
        let host = tcpTestHost.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !host.isEmpty, !isTestingTcp else { return }
        let port = UInt16(tcpTestPort) ?? 22
        let timeout = UInt32(tcpTestTimeout) ?? 2000
        isTestingTcp = true
        tcpTestResult = "テスト中…"
        tcpTestSuccess = nil

        Task.detached(priority: .userInitiated) {
            let res = Self.testTCP(host: host, port: port, timeoutMs: timeout)
            await MainActor.run {
                self.isTestingTcp = false
                self.tcpTestSuccess = res.success
                self.tcpTestResult = res.message
                let timeStr = DateFormatter.localizedString(from: Date(), dateStyle: .none, timeStyle: .medium)
                let icon = res.success ? "🟢" : "🔴"
                let record = "\(icon) [\(timeStr)] \(host):\(port) -> \(res.message)"
                self.recentTcpTests.insert(record, at: 0)
                if self.recentTcpTests.count > 15 { self.recentTcpTests.removeLast() }
            }
        }
    }

    func importLegacyDevices(fromJSON data: Data) throws -> LegacyConnectionImportResult {
        let result = try LegacyConnectionImporter.importJSON(data, existing: connections)
        connections.append(contentsOf: result.imported)
        return result
    }
}
