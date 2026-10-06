import Foundation

public struct ChatMessage: Identifiable, Codable, Equatable {
    public enum Role: String, Codable { case user, assistant }

    public var id: UUID
    public var role: Role
    public var text: String
    public var agentGoal: String?
    public var agentProgress: [AgentProgressEntry]?
    public var attachments: [String]
    public var probeResults: [AgentToolResult]?

    public var hasProbeResults: Bool { !(probeResults ?? []).isEmpty }
    public var displayedProbeResults: [AgentToolResult] {
        var seen = Set<String>()
        return (probeResults ?? []).reversed().filter {
            seen.insert($0.probeDisplayName ?? $0.tool).inserted
        }.reversed()
    }

    public var conversationText: String {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        let plannerErrors = ["エラー: action Observe requires a tool or target", "エラー: action Verify requires a tool or target", "エラー: planner omitted a read-only tool"]
        if plannerErrors.contains(trimmed),
           let summary = (probeResults ?? []).reversed().compactMap({ ProbeResultPresentation.pingStatisticsSummary($0) }).first {
            return summary
        }
        if hasProbeResults,
           ProbeResultPresentation.legacyResult(from: text, messageID: id) != nil
            || (probeResults ?? []).contains(where: {
                $0.output.trimmingCharacters(in: .whitespacesAndNewlines) == trimmed
            }) {
            return ""
        }
        return text
    }

    public init(id: UUID = UUID(), role: Role, text: String, attachments: [String] = []) {
        self.id = id
        self.role = role
        self.text = text
        self.attachments = attachments
    }

    private enum CodingKeys: String, CodingKey { case id, role, text, attachments, agentGoal, agentProgress, probeResults }

    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        id = try values.decodeIfPresent(UUID.self, forKey: .id) ?? UUID()
        role = try values.decode(Role.self, forKey: .role)
        text = try values.decode(String.self, forKey: .text)
        agentGoal = try values.decodeIfPresent(String.self, forKey: .agentGoal)
        agentProgress = try values.decodeIfPresent([AgentProgressEntry].self, forKey: .agentProgress)
        attachments = try values.decodeIfPresent([String].self, forKey: .attachments) ?? []
        probeResults = try values.decodeIfPresent([AgentToolResult].self, forKey: .probeResults)
        if role == .assistant, probeResults == nil,
           let legacy = ProbeResultPresentation.legacyResult(from: text, messageID: id) {
            probeResults = [legacy]
        }
    }
}

public enum GreetingPresentation {
    public static func isGreeting(_ text: String) -> Bool {
        let normalized = text.trimmingCharacters(in: .whitespacesAndNewlines)
            .lowercased()
            .trimmingCharacters(in: CharacterSet(charactersIn: "!！?？。、,. "))
        return ["こんにちは", "こんばんは", "おはよう", "おはようございます", "やあ", "hello", "hi", "hey"].contains(normalized)
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

public struct ChatSessionState: Equatable, Codable {
    public var sessions: [ChatSession]
    public var activeSessionID: UUID?
    public init(sessions: [ChatSession] = [], activeSessionID: UUID? = nil) {
        self.sessions = sessions
        self.activeSessionID = activeSessionID
        apply("normalize")
    }
    private mutating func apply(_ action: String, id: UUID? = nil, title: String? = nil) {
        var request: [String: Any] = ["op": "sessions", "action": action, "state": RustPolicy.object(self),
            "now": Date().timeIntervalSinceReferenceDate]
        if let id { request["id"] = id.uuidString }
        if let title { request["title"] = title }
        self = RustPolicy.call(request)
    }
    @discardableResult public mutating func create(title: String = "新しい会話") -> ChatSession {
        apply("create", title: title)
        return sessions[0]
    }
    public mutating func select(_ id: UUID) { apply("select", id: id) }
    public mutating func delete(_ id: UUID) { apply("delete", id: id) }
    public mutating func rename(_ id: UUID, to title: String) { apply("rename", id: id, title: title) }
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

    private struct Policy: Decodable {
        let error: String?
        let defaultPort: String
        let effectivePort: String
        let driver: String
    }
    private func policy(base: String = "") -> Policy {
        RustPolicy.call(["op": "connection", "connection": RustPolicy.object(self), "base": base])
    }
    public var validationError: String? { policy().error }
    public var defaultPort: String { policy().defaultPort }
    public var effectivePort: String { policy().effectivePort }
    public mutating func selectConnectionType(_ type: String) {
        self = RustPolicy.call(["op": "connection_select_type", "connection": RustPolicy.object(self), "type": type])
    }
    public func transportDeviceType(_ base: String) -> String { policy(base: base).driver }

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

public enum ConnectionInventoryPolicy {
    public static func saving(_ connection: SavedConnection, into connections: [SavedConnection]) -> [SavedConnection] {
        guard connection.validationError == nil else { return connections }
        return RustPolicy.call(["op": "connection_save", "connection": RustPolicy.object(connection),
            "connections": RustPolicy.object(connections)])
    }

    public static func removing(_ id: UUID, from connections: [SavedConnection]) -> [SavedConnection] {
        RustPolicy.call(["op": "connection_remove", "id": id.uuidString, "connections": RustPolicy.object(connections)])
    }
}

public enum ConnectionCredentialPolicy {
    public static func applying(
        password: String?,
        enablePassword: String?,
        to connection: SavedConnection
    ) -> SavedConnection {
        var request: [String: Any] = ["op": "credential_flags", "connection": RustPolicy.object(connection)]
        if let password { request["passwordPresent"] = !password.isEmpty }
        if let enablePassword { request["enablePresent"] = !enablePassword.isEmpty }
        return RustPolicy.call(request)
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
