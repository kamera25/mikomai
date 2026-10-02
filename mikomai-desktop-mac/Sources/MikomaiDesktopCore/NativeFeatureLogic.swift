import Foundation
import Darwin

public struct ChatMessage: Identifiable, Codable, Equatable {
    public enum Role: String, Codable { case user, assistant }

    public var id: UUID
    public var role: Role
    public var text: String
    public var agentGoal: String?
    public var agentProgress: [AgentProgressEntry]?
    public var attachments: [String]

    public init(id: UUID = UUID(), role: Role, text: String, attachments: [String] = []) {
        self.id = id
        self.role = role
        self.text = text
        self.attachments = attachments
    }

    private enum CodingKeys: String, CodingKey { case id, role, text, attachments, agentGoal, agentProgress }

    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        id = try values.decodeIfPresent(UUID.self, forKey: .id) ?? UUID()
        role = try values.decode(Role.self, forKey: .role)
        text = try values.decode(String.self, forKey: .text)
        agentGoal = try values.decodeIfPresent(String.self, forKey: .agentGoal)
        agentProgress = try values.decodeIfPresent([AgentProgressEntry].self, forKey: .agentProgress)
        attachments = try values.decodeIfPresent([String].self, forKey: .attachments) ?? []
    }
}

public struct ChatSession: Identifiable, Codable, Equatable {
    public var id: UUID
    public var title: String
    public var messages: [ChatMessage]
    public var updatedAt: Date

    public init(id: UUID = UUID(), title: String, messages: [ChatMessage] = [], updatedAt: Date = Date()) {
        self.id = id
        self.title = title
        self.messages = messages
        self.updatedAt = updatedAt
    }
}

public struct ChatSessionState: Equatable {
    public var sessions: [ChatSession]
    public var activeSessionID: UUID?

    public init(sessions: [ChatSession] = [], activeSessionID: UUID? = nil) {
        self.sessions = sessions
        self.activeSessionID = sessions.contains(where: { $0.id == activeSessionID })
            ? activeSessionID
            : sessions.first?.id
    }

    @discardableResult
    public mutating func create(title: String = "新しい会話") -> ChatSession {
        let session = ChatSession(title: title)
        sessions.insert(session, at: 0)
        activeSessionID = session.id
        return session
    }

    public mutating func select(_ id: UUID) {
        guard sessions.contains(where: { $0.id == id }) else { return }
        activeSessionID = id
    }

    public mutating func delete(_ id: UUID) {
        sessions.removeAll { $0.id == id }
        if activeSessionID == id { activeSessionID = sessions.first?.id }
        if sessions.isEmpty { create() }
    }

    public mutating func rename(_ id: UUID, to title: String) {
        let normalized = title.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !normalized.isEmpty, let index = sessions.firstIndex(where: { $0.id == id }) else { return }
        sessions[index].title = normalized
    }
}

public struct PendingAttachment: Identifiable, Equatable {
    public let id: UUID
    public let name: String
    public let text: String
    public let imageData: Data?
    public let mimeType: String?
    public var byteCount: Int { imageData?.count ?? text.utf8.count }

    public init(id: UUID = UUID(), name: String, text: String, imageData: Data? = nil, mimeType: String? = nil) {
        self.id = id
        self.name = name
        self.text = text
        self.imageData = imageData
        self.mimeType = mimeType
    }
}

public enum ChatSubmissionPolicy {
    public static func normalizedPrompt(_ prompt: String) -> String {
        prompt.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    public static func hasContent(prompt: String, attachmentCount: Int) -> Bool {
        !normalizedPrompt(prompt).isEmpty || attachmentCount > 0
    }

    public static func shouldSubmit(prompt: String, attachmentCount: Int, isWorking: Bool) -> Bool {
        hasContent(prompt: prompt, attachmentCount: attachmentCount)
    }

    public static func canStop(isWorking: Bool, isCancelling: Bool) -> Bool {
        isWorking && !isCancelling
    }
}

public struct ChatSuggestionVisibilityState: Equatable {
    public private(set) var isVisible = false

    public init() {}

    public mutating func updateForInput(hasMentionQuery: Bool, candidateCount: Int) {
        isVisible = hasMentionQuery && candidateCount > 0
    }

    public mutating func updateCandidates(count: Int) {
        if count == 0 { isVisible = false }
    }

    public mutating func dismissForEscape() {
        isVisible = false
    }
}

public enum AttachmentReadError: LocalizedError, Equatable {
    case unsupportedType
    case duplicate
    case tooLarge
    case totalTooLarge
    case invalidEncoding
    case containsNull

    public var errorDescription: String? {
        switch self {
        case .unsupportedType: "この形式のファイルは添付できません。"
        case .duplicate: "同じ名前のファイルは添付済みです。"
        case .tooLarge: "ファイルは64 KiB以下にしてください。"
        case .totalTooLarge: "添付ファイルの合計は128 KiB以下にしてください。"
        case .invalidEncoding: "UTF-8テキストではありません。"
        case .containsNull: "NUL文字を含むファイルは添付できません。"
        }
    }
}

public enum TextAttachmentPolicy {
    public static let maxFileBytes = 64 * 1024
    public static let maxTotalBytes = 128 * 1024
    private static let supportedExtensions: Set<String> = ["txt", "md", "csv", "json", "yaml", "yml", "xml", "log"]

    public static func prepare(
        name: String,
        data: Data,
        existingNames: Set<String> = [],
        currentTotalBytes: Int = 0
    ) throws -> PendingAttachment {
        guard supportedExtensions.contains(URL(fileURLWithPath: name).pathExtension.lowercased()) else {
            throw AttachmentReadError.unsupportedType
        }
        guard !existingNames.contains(name) else { throw AttachmentReadError.duplicate }
        guard data.count <= maxFileBytes else { throw AttachmentReadError.tooLarge }
        guard currentTotalBytes + data.count <= maxTotalBytes else { throw AttachmentReadError.totalTooLarge }
        guard let text = String(data: data, encoding: .utf8) else { throw AttachmentReadError.invalidEncoding }
        guard !text.unicodeScalars.contains(where: { $0.value == 0 }) else { throw AttachmentReadError.containsNull }
        return PendingAttachment(name: name, text: text)
    }
}

public enum AttachmentMediaPolicy {
    private static let imageExtensions: Set<String> = [
        "png", "jpg", "jpeg", "gif", "webp", "bmp", "svg", "heic", "heif", "tiff"
    ]

    public static func isImagePath(_ path: String) -> Bool {
        imageExtensions.contains(URL(fileURLWithPath: path).pathExtension.lowercased())
    }

    public static func isImageFile(name: String, mediaType: String?) -> Bool {
        mediaType?.hasPrefix("image/") == true || isImagePath(name)
    }
}

public struct SavedConnection: Identifiable, Codable, Equatable, Sendable {
    public var id: UUID
    public var sourceID: String?
    public var name: String
    public var host: String
    public var port: String
    public var connectionType: String?
    public var username: String
    public var deviceType: String
    public var hasPassword: Bool
    public var hasEnablePassword: Bool

    public init(
        id: UUID = UUID(), sourceID: String? = nil, name: String, host: String, port: String = "22",
        connectionType: String? = "SSH", username: String = "", deviceType: String = "cisco_ios", hasPassword: Bool = false,
        hasEnablePassword: Bool = false
    ) {
        self.id = id
        self.sourceID = sourceID
        self.name = name
        self.host = host
        self.port = port
        self.connectionType = connectionType
        self.username = username
        self.deviceType = deviceType
        self.hasPassword = hasPassword
        self.hasEnablePassword = hasEnablePassword
    }

    public var validationError: String? {
        if !Self.isSafeHostname(name) { return "名前は文字・数字と . - _ で入力してください。" }
        if !Self.isSafeHost(host) { return "ホストは IP アドレスまたは文字・数字と . - _ で入力してください。" }
        if !port.isEmpty && (!(Int(port).map { (1...65535).contains($0) } ?? false)) { return "ポートは 1 から 65535 の数値で入力してください。" }
        if username.count > 128 || Self.containsControl(username) { return "ユーザー名が長すぎるか、使用できない文字を含んでいます。" }
        if deviceType.isEmpty || deviceType.count > 128 || Self.containsControl(deviceType) { return "機器タイプは 1 から 128 文字で入力してください。" }
        return nil
    }

    public var defaultPort: String {
        (connectionType ?? "SSH").lowercased() == "telnet" ? "23" : "22"
    }

    public var effectivePort: String { port.isEmpty ? defaultPort : port }

    public mutating func selectConnectionType(_ type: String) {
        let usesDefaultPort = port.isEmpty || port == defaultPort
        connectionType = type
        if usesDefaultPort { port = defaultPort }
    }

    public func transportDeviceType(_ base: String) -> String {
        guard (connectionType ?? "SSH").lowercased() == "telnet", !base.hasSuffix("_telnet") else { return base }
        return (base.hasSuffix("_ssh") ? String(base.dropLast(4)) : base) + "_telnet"
    }

    private static func isSafeHostname(_ value: String) -> Bool {
        !value.isEmpty && value.count <= 255 && value.allSatisfy { $0.isLetter || $0.isNumber || ".-_".contains($0) }
    }

    private static func isSafeHost(_ value: String) -> Bool {
        if value.isEmpty || value.count > 255 || containsControl(value) { return false }
        if value.contains(":") {
            var address = in6_addr()
            return value.withCString { inet_pton(AF_INET6, $0, &address) == 1 }
        }
        return value.allSatisfy { $0.isLetter || $0.isNumber || ".-_".contains($0) }
    }

    private static func containsControl(_ value: String) -> Bool {
        value.unicodeScalars.contains(where: CharacterSet.controlCharacters.contains)
    }
}

public struct HostSuggestion: Equatable, Identifiable, Sendable {
    public var hostname: String
    public var ip: String
    public var id: String { "\(hostname)\u{0}\(ip)" }

    public init(hostname: String, ip: String) {
        self.hostname = hostname
        self.ip = ip
    }
}

public struct HostSuggestionLabels: Equatable {
    public var localhost: String
    public var pastIps: String

    public init(localhost: String, pastIps: String) {
        self.localhost = localhost
        self.pastIps = pastIps
    }
}

public enum HostSuggestionPolicy {
    public static func find(
        query: String,
        availableHosts: [HostSuggestion],
        recentIPs: [String],
        labels: HostSuggestionLabels
    ) -> [HostSuggestion] {
        let loweredQuery = query.lowercased()
        var suggestions: [HostSuggestion] = []
        var seenIPs = Set<String>()

        if query.isEmpty || "localhost".contains(loweredQuery) || labels.localhost.contains(query) {
            suggestions.append(HostSuggestion(hostname: "localhost", ip: labels.localhost))
            seenIPs.formUnion(["127.0.0.1", "localhost"])
        }

        for host in availableHosts {
            if host.hostname != "localhost",
               (query.isEmpty || host.hostname.lowercased().contains(loweredQuery) || host.ip.contains(query)) {
                suggestions.append(host)
            }
            seenIPs.insert(host.ip)
        }

        for ip in recentIPs where (query.isEmpty || ip.lowercased().contains(loweredQuery) || labels.pastIps.contains(query)) && !seenIPs.contains(ip) {
            suggestions.append(HostSuggestion(hostname: ip, ip: labels.pastIps))
            seenIPs.insert(ip)
        }
        return suggestions
    }

    public static func updateRecentHosts(_ hosts: [String], current: [String], limit: Int = 10) -> [String] {
        guard !hosts.isEmpty else { return current }
        var seen = Set<String>()
        return (hosts + current).filter { seen.insert($0).inserted }.prefix(max(0, limit)).map { $0 }
    }
}

public struct LegacyDeviceSummary: Decodable {
    public var id: String?
    public var hostname: String
    public var ip: String?
    public var port: String?
    public var connectionType: String?
    public var deviceType: String?

    private enum CodingKeys: String, CodingKey {
        case id, hostname, ip, port, deviceType
        case connectionType = "type"
    }

    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        id = try values.decodeIfPresent(String.self, forKey: .id)
        hostname = try values.decode(String.self, forKey: .hostname)
        ip = try values.decodeIfPresent(String.self, forKey: .ip)
        if let numericPort = try? values.decodeIfPresent(Int.self, forKey: .port) {
            port = String(numericPort)
        } else {
            port = try values.decodeIfPresent(String.self, forKey: .port)
        }
        connectionType = try values.decodeIfPresent(String.self, forKey: .connectionType)
        deviceType = try values.decodeIfPresent(String.self, forKey: .deviceType)
    }
}

public struct LegacyConnectionImportResult {
    public let imported: [SavedConnection]
    public let skipped: Int
    public let missingIDs: Int
}

public enum ConnectionInventoryPolicy {
    public static func saving(_ connection: SavedConnection, into connections: [SavedConnection]) -> [SavedConnection] {
        guard connection.validationError == nil else { return connections }
        var updated = connections
        if let index = updated.firstIndex(where: { $0.id == connection.id }) {
            updated[index] = connection
        } else {
            updated.append(connection)
        }
        return updated
    }

    public static func removing(_ id: UUID, from connections: [SavedConnection]) -> [SavedConnection] {
        connections.filter { $0.id != id }
    }
}

public enum ConnectionCredentialPolicy {
    public static func applying(
        password: String?,
        enablePassword: String?,
        to connection: SavedConnection
    ) -> SavedConnection {
        var updated = connection
        if let password { updated.hasPassword = !password.isEmpty }
        if let enablePassword { updated.hasEnablePassword = !enablePassword.isEmpty }
        return updated
    }
}

public protocol CredentialStore: Sendable {
    func save(key: String, value: String)
    func load(key: String) -> String?
    func delete(key: String)
}

public struct ConnectionCredentials: Equatable, Sendable {
    public let password: String?
    public let enablePassword: String?

    public init(password: String?, enablePassword: String?) {
        self.password = password
        self.enablePassword = enablePassword
    }
}

public struct ConnectionCredentialPersistence: Sendable {
    private let store: any CredentialStore

    public init(store: any CredentialStore) {
        self.store = store
    }

    public func load(for connectionID: UUID) -> ConnectionCredentials {
        ConnectionCredentials(
            password: store.load(key: key(for: connectionID, credential: "password")),
            enablePassword: store.load(key: key(for: connectionID, credential: "enable"))
        )
    }

    @discardableResult
    public func save(
        for connectionID: UUID,
        password: String?,
        enablePassword: String?
    ) -> ConnectionCredentials {
        update(password, key: key(for: connectionID, credential: "password"))
        update(enablePassword, key: key(for: connectionID, credential: "enable"))
        return load(for: connectionID)
    }

    public func delete(for connectionID: UUID) {
        store.delete(key: key(for: connectionID, credential: "password"))
        store.delete(key: key(for: connectionID, credential: "enable"))
    }

    private func update(_ value: String?, key: String) {
        guard let value else { return }
        if value.isEmpty {
            store.delete(key: key)
        } else {
            store.save(key: key, value: value)
        }
    }

    private func key(for connectionID: UUID, credential: String) -> String {
        "conn.\(connectionID.uuidString).\(credential)"
    }
}

public enum LegacyConnectionImporter {
    public static func importJSON(_ data: Data, existing: [SavedConnection]) throws -> LegacyConnectionImportResult {
        let raw = try JSONSerialization.jsonObject(with: data)
        guard let rows = raw as? [Any] else {
            throw DecodingError.typeMismatch([LegacyDeviceSummary].self, .init(codingPath: [], debugDescription: "Expected an array of device records"))
        }

        var knownIDs = Set(existing.compactMap(\.sourceID))
        var imported: [SavedConnection] = []
        var skipped = 0
        var missingIDs = 0
        let decoder = JSONDecoder()
        for row in rows {
            guard JSONSerialization.isValidJSONObject(row), let rowData = try? JSONSerialization.data(withJSONObject: row),
                  let device = try? decoder.decode(LegacyDeviceSummary.self, from: rowData) else {
                skipped += 1
                continue
            }
            if device.id == nil { missingIDs += 1 }
            if let id = device.id, knownIDs.contains(id) {
                skipped += 1
                continue
            }
            let hostname = device.hostname.trimmingCharacters(in: .whitespacesAndNewlines)
            let host = device.ip.flatMap { $0.isEmpty ? nil : $0 } ?? hostname
            let connection = SavedConnection(
                sourceID: device.id,
                name: hostname,
                host: host,
                port: device.port ?? "22",
                connectionType: device.connectionType ?? "SSH",
                deviceType: device.deviceType ?? device.connectionType ?? "不明"
            )
            guard connection.validationError == nil else { skipped += 1; continue }
            if let id = device.id { knownIDs.insert(id) }
            imported.append(connection)
        }
        return LegacyConnectionImportResult(imported: imported, skipped: skipped, missingIDs: missingIDs)
    }
}

public struct ConnectionCSVWarning: Equatable {
    public let row: Int
    public let reason: String
}

public struct ConnectionCSVImportResult {
    public let connections: [SavedConnection]
    public let importedCount: Int
    public let warnings: [ConnectionCSVWarning]
}

public enum ConnectionCSVError: LocalizedError, Equatable {
    case malformed
    case missingHeader
    case invalidExportRecord(String)

    public var errorDescription: String? {
        switch self {
        case .malformed: "CSV の引用符または行形式を確認してください。"
        case .missingHeader: "CSV のヘッダー行がありません。"
        case let .invalidExportRecord(reason): reason
        }
    }
}

public enum ConnectionCSVCodec {
    public static let headers = [
        "id", "status", "hostname", "ip", "port", "type", "lastConnected", "deviceType", "vendorType", "username"
    ]

    public static func importCSV(_ input: String, existing: [SavedConnection]) throws -> ConnectionCSVImportResult {
        let records = try parse(input)
        guard let rawHeader = records.first else { throw ConnectionCSVError.missingHeader }
        var header = rawHeader.map { $0.trimmingCharacters(in: .whitespacesAndNewlines).lowercased() }
        if let first = header.first { header[0] = first.trimmingCharacters(in: CharacterSet(charactersIn: "\u{FEFF}")) }
        guard !header.isEmpty else { throw ConnectionCSVError.missingHeader }

        var columns: [String: Int] = [:]
        for (index, name) in header.enumerated() where columns[name] == nil { columns[name] = index }
        func value(_ row: [String], _ key: String, fallback: String = "") -> String {
            guard let index = columns[key], row.indices.contains(index) else { return fallback }
            return row[index].trimmingCharacters(in: .whitespacesAndNewlines)
        }

        var merged = existing
        var importedCount = 0
        var warnings: [ConnectionCSVWarning] = []
        for (offset, row) in records.dropFirst().enumerated() {
            let rowNumber = offset + 2
            if row.allSatisfy({ $0.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty }) { continue }
            func reject(_ reason: String) { warnings.append(ConnectionCSVWarning(row: rowNumber, reason: reason)) }

            let name = value(row, "hostname", fallback: value(row, "name"))
            let host = value(row, "ip", fallback: value(row, "host"))
            guard !name.isEmpty, !host.isEmpty else {
                reject("hostname と ip は必須です")
                continue
            }

            let username = value(row, "username")
            let deviceType = value(row, "devicetype", fallback: "Cisco IOS")
            guard username.utf8.count <= 128 else { reject("username exceeds max length of 128"); continue }
            guard deviceType.utf8.count <= 128 else { reject("deviceType exceeds max length of 128"); continue }
            guard !containsForbiddenControl(username) else { reject("username contains forbidden control character"); continue }
            guard !containsForbiddenControl(deviceType) else { reject("deviceType contains forbidden control character"); continue }

            let rawType = value(row, "type")
            guard let connectionType = canonicalConnectionType(rawType) else {
                reject("未対応の接続タイプです: '\(rawType)'")
                continue
            }
            let id = value(row, "id").isEmpty ? UUID().uuidString : value(row, "id")
            let connection = SavedConnection(
                sourceID: id,
                name: name,
                host: host,
                port: value(row, "port"),
                connectionType: connectionType,
                username: username,
                deviceType: deviceType
            )
            guard let reason = connection.validationError else {
                importedCount += 1
                if let existingIndex = merged.firstIndex(where: {
                    $0.sourceID == id || $0.id.uuidString.caseInsensitiveCompare(id) == .orderedSame
                }) {
                    var updated = connection
                    updated.id = merged[existingIndex].id
                    updated.hasPassword = merged[existingIndex].hasPassword
                    updated.hasEnablePassword = merged[existingIndex].hasEnablePassword
                    merged[existingIndex] = updated
                } else {
                    merged.append(connection)
                }
                continue
            }
            reject(reason)
        }
        return ConnectionCSVImportResult(connections: merged, importedCount: importedCount, warnings: warnings)
    }

    public static func exportCSV(_ connections: [SavedConnection]) throws -> String {
        var records = [headers]
        for connection in connections {
            if let reason = connection.validationError { throw ConnectionCSVError.invalidExportRecord(reason) }
            let rawType = connection.connectionType ?? "SSH"
            guard let connectionType = canonicalConnectionType(rawType) else {
                throw ConnectionCSVError.invalidExportRecord("未対応の接続タイプです: '\(rawType)'")
            }
            records.append([
                connection.sourceID ?? connection.id.uuidString,
                "offline",
                connection.name,
                connection.host,
                connection.port,
                connectionType,
                "Never",
                connection.deviceType,
                "",
                connection.username
            ])
        }
        return records.map { $0.map(escape).joined(separator: ",") }.joined(separator: "\n") + "\n"
    }

    private static func canonicalConnectionType(_ value: String) -> String? {
        let normalized = value.lowercased()
        if normalized.contains("console") || normalized.contains("serial") { return "Console" }
        if normalized.contains("telnet") { return "Telnet" }
        if normalized.contains("ssh") { return "SSH" }
        if value.isEmpty { return "SSH" }
        return nil
    }

    private static func containsForbiddenControl(_ value: String) -> Bool {
        value.unicodeScalars.contains { CharacterSet.controlCharacters.contains($0) }
    }

    private static func escape(_ value: String) -> String {
        guard value.contains(where: { ",\"\r\n".contains($0) }) else { return value }
        return "\"\(value.replacingOccurrences(of: "\"", with: "\"\""))\""
    }

    private static func parse(_ input: String) throws -> [[String]] {
        let bytes = Array(input.utf8)
        var records: [[String]] = []
        var row: [String] = []
        var field: [UInt8] = []
        var insideQuotes = false
        var afterClosingQuote = false
        var index = 0

        while index < bytes.count {
            let byte = bytes[index]
            if insideQuotes {
                if byte == 34 {
                    if index + 1 < bytes.count && bytes[index + 1] == 34 {
                        field.append(34)
                        index += 1
                    } else {
                        insideQuotes = false
                        afterClosingQuote = true
                    }
                } else {
                    field.append(byte)
                }
            } else if afterClosingQuote {
                if byte == 44 {
                    row.append(String(decoding: field, as: UTF8.self))
                    field = []
                    afterClosingQuote = false
                } else if byte == 10 || byte == 13 {
                    row.append(String(decoding: field, as: UTF8.self))
                    records.append(row)
                    row = []
                    field = []
                    afterClosingQuote = false
                    if byte == 13, index + 1 < bytes.count, bytes[index + 1] == 10 { index += 1 }
                } else if byte == 32 || byte == 9 {
                    // Ignore spaces outside a quoted cell.
                } else {
                    throw ConnectionCSVError.malformed
                }
            } else if byte == 34 {
                guard field.isEmpty else { throw ConnectionCSVError.malformed }
                insideQuotes = true
            } else if byte == 44 {
                row.append(String(decoding: field, as: UTF8.self))
                field = []
            } else if byte == 10 || byte == 13 {
                row.append(String(decoding: field, as: UTF8.self))
                records.append(row)
                row = []
                field = []
                if byte == 13, index + 1 < bytes.count, bytes[index + 1] == 10 { index += 1 }
            } else {
                field.append(byte)
            }
            index += 1
        }

        guard !insideQuotes else { throw ConnectionCSVError.malformed }
        if afterClosingQuote || !field.isEmpty || !row.isEmpty {
            row.append(String(decoding: field, as: UTF8.self))
            records.append(row)
        }
        return records
    }
}


public enum ImageAttachmentError: LocalizedError {
    case disabled, invalidImage, tooLarge, tooMany, totalTooLarge
    public var errorDescription: String? {
        switch self {
        case .disabled: "画像を添付するには設定でVisionを有効にし、対応するmmprojを指定してください。"
        case .invalidImage: "PNGまたはJPEGの画像を選択してください。"
        case .tooLarge: "画像は1ファイル8 MiB以下にしてください。"
        case .tooMany: "画像は4ファイル以下にしてください。"
        case .totalTooLarge: "画像の合計は16 MiB以下にしてください。"
        }
    }
}
public enum ImageAttachmentPolicy {
    public static let maxFileBytes = 8 * 1024 * 1024
    public static let maxTotalBytes = 16 * 1024 * 1024
    public static func isImage(name: String) -> Bool { ["png", "jpg", "jpeg"].contains(URL(fileURLWithPath: name).pathExtension.lowercased()) }
    public static func prepare(name: String, data: Data, existing: [PendingAttachment], visionEnabled: Bool) throws -> PendingAttachment {
        guard visionEnabled else { throw ImageAttachmentError.disabled }
        guard !existing.contains(where: { $0.name == name }) else { throw AttachmentReadError.duplicate }
        guard data.count <= maxFileBytes else { throw ImageAttachmentError.tooLarge }
        let images = existing.filter { $0.imageData != nil }
        guard images.count < 4 else { throw ImageAttachmentError.tooMany }
        guard images.reduce(0, { $0 + $1.byteCount }) + data.count <= maxTotalBytes else { throw ImageAttachmentError.totalTooLarge }
        let ext = URL(fileURLWithPath: name).pathExtension.lowercased()
        let mime: String
        if ext == "png" && data.starts(with: [0x89,0x50,0x4e,0x47,0x0d,0x0a,0x1a,0x0a]) { mime = "image/png" }
        else if ["jpg", "jpeg"].contains(ext) && data.starts(with: [0xff,0xd8]) { mime = "image/jpeg" }
        else { throw ImageAttachmentError.invalidImage }
        return PendingAttachment(name: name, text: "", imageData: data, mimeType: mime)
    }
}
public enum NativeAttachmentPayload {
    private struct Image: Encodable { let name: String; let mimeType: String; let base64: String }
    private struct Payload: Encodable { let text: String; let images: [Image] }
    public static func encode(_ attachments: [PendingAttachment]) throws -> String {
        let text = attachments.filter { $0.imageData == nil }.enumerated().map { offset, attachment in
            "[添付ファイル \(offset + 1): \(attachment.name)]\n\(attachment.text)"
        }.joined(separator: "\n\n")
        let images = attachments.compactMap { attachment -> Image? in
            guard let data = attachment.imageData else { return nil }
            return Image(name: attachment.name, mimeType: attachment.mimeType ?? "", base64: data.base64EncodedString())
        }
        if images.isEmpty { return text }
        return "__MIKOMAI_ATTACHMENTS_V1__" + String(decoding: try JSONEncoder().encode(Payload(text: text, images: images)), as: UTF8.self)
    }
}
