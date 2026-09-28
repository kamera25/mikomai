import SwiftUI
import AppKit
import Darwin
import Security
import MikomaiFFI
import UniformTypeIdentifiers

@main
struct MikomaiDesktopMac: App {
    @StateObject private var model = DesktopModel()

    var body: some Scene {
        WindowGroup {
            DesktopWindow(model: model)
                .frame(minWidth: 980, minHeight: 650)
        }
        .windowStyle(.hiddenTitleBar)
        .commands {
            CommandGroup(replacing: .newItem) {
                Button("新しい会話") {
                    model.createSession()
                }
                .keyboardShortcut("n", modifiers: .command)
            }
            CommandMenu("表示") {
                Button("チャット") { model.workspace = .chat }.keyboardShortcut("1", modifiers: .command)
                Button("機器情報一覧") { model.workspace = .connections }.keyboardShortcut("2", modifiers: .command)
                Button("ネットワークツール") { model.workspace = .tools }.keyboardShortcut("3", modifiers: .command)
                Button("設定") { model.workspace = .settings }.keyboardShortcut("4", modifiers: .command)
            }
            CommandMenu("ネットワーク") {
                Button("接続テスト") {
                    model.workspace = .tools
                    model.selectedToolTab = .tcpTest
                }
                Button("Ping / Trace を実行") {
                    model.workspace = .tools
                    model.selectedToolTab = .ping
                }
                Button("ARP テーブル表示") {
                    model.workspace = .tools
                    model.selectedToolTab = .arp
                }
                Button("ルーティングテーブル表示") {
                    model.workspace = .tools
                    model.selectedToolTab = .route
                }
            }
        }
    }
}

// MARK: - Enums & Models

private enum Workspace: String, CaseIterable, Identifiable {
    case chat = "チャット"
    case connections = "機器情報一覧"
    case tools = "ネットワークツール"
    case settings = "設定"

    var id: String { rawValue }
    var icon: String {
        switch self {
        case .chat: "bubble.left.and.bubble.right"
        case .connections: "point.3.connected.trianglepath.dotted"
        case .tools: "wrench.and.screwdriver"
        case .settings: "gearshape"
        }
    }
}

private enum ToolTab: String, CaseIterable, Identifiable {
    case tcpTest = "接続テスト"
    case ping = "Ping / Trace"
    case arp = "ARP テーブル"
    case route = "ルーティング"

    var id: String { rawValue }
    var icon: String {
        switch self {
        case .tcpTest: "bolt.horizontal.fill"
        case .ping: "antenna.radiowaves.left.and.right"
        case .arp: "tablecells"
        case .route: "arrow.triangle.branch"
        }
    }
}

private struct ChatMessage: Identifiable, Codable {
    enum Role: String, Codable { case user, assistant }
    var id = UUID()
    var role: Role
    var text: String
    var attachments: [String] = []

    private enum CodingKeys: String, CodingKey { case id, role, text, attachments }

    init(id: UUID = UUID(), role: Role, text: String, attachments: [String] = []) {
        self.id = id
        self.role = role
        self.text = text
        self.attachments = attachments
    }

    init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        id = try values.decodeIfPresent(UUID.self, forKey: .id) ?? UUID()
        role = try values.decode(Role.self, forKey: .role)
        text = try values.decode(String.self, forKey: .text)
        attachments = try values.decodeIfPresent([String].self, forKey: .attachments) ?? []
    }
}

private struct PendingAttachment: Identifiable {
    let id = UUID()
    let name: String
    let text: String
    var byteCount: Int { text.utf8.count }
}

private struct ChatSession: Identifiable, Codable {
    var id = UUID()
    var title: String
    var messages: [ChatMessage] = []
    var updatedAt = Date()
}

private struct SavedConnection: Identifiable, Codable {
    var id = UUID()
    var sourceID: String? = nil
    var name: String
    var host: String
    var port = "22"
    var username = ""
    var deviceType = "Cisco IOS"
    var hasPassword: Bool = false
    var hasEnablePassword: Bool = false

    var validationError: String? {
        if !Self.isSafeHostname(name) { return "名前は文字・数字と . - _ で入力してください。" }
        if !Self.isSafeHost(host) { return "ホストは IP アドレスまたは文字・数字と . - _ で入力してください。" }
        if !port.isEmpty && (!(Int(port).map { (1...65535).contains($0) } ?? false)) { return "ポートは 1 から 65535 の数値で入力してください。" }
        if username.count > 128 || Self.containsControl(username) { return "ユーザー名が長すぎるか、使用できない文字を含んでいます。" }
        if deviceType.isEmpty || deviceType.count > 100 || Self.containsControl(deviceType) { return "機器タイプは 1 から 100 文字で入力してください。" }
        return nil
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

private struct ConnectionTestStatus {
    let success: Bool
    let message: String
    let latencyMs: Int?
    let timestamp: Date
}

private struct ArpRecord: Identifiable {
    let id = UUID()
    let ip: String
    let mac: String
    let interface: String
    let isPermanent: Bool
    let isIncomplete: Bool
}

private struct RouteRecord: Identifiable {
    let id = UUID()
    let destination: String
    let gateway: String
    let flags: String
    let interface: String
}

private struct TauriDeviceSummary: Decodable {
    var id: String?
    var hostname: String
    var ip: String?
    var port: String?
    var connectionType: String?
    var deviceType: String?

    private enum CodingKeys: String, CodingKey {
        case id, hostname, ip, port, deviceType
        case connectionType = "type"
    }
}

// MARK: - Keychain Helper

private enum KeychainHelper {
    private static let service = "com.mikomai.desktop.mac"

    static func save(key: String, value: String) {
        guard let data = value.data(using: .utf8) else { return }
        delete(key: key)
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: key,
            kSecValueData as String: data,
            kSecAttrAccessible as String: kSecAttrAccessibleAfterFirstUnlock
        ]
        SecItemAdd(query as CFDictionary, nil)
    }

    static func load(key: String) -> String? {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: key,
            kSecReturnData as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne
        ]
        var result: AnyObject?
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        guard status == errSecSuccess, let data = result as? Data else { return nil }
        return String(data: data, encoding: .utf8)
    }

    static func delete(key: String) {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: key
        ]
        SecItemDelete(query as CFDictionary)
    }
}

// MARK: - C ABI Streaming Callback Bridge

private final class StreamBox: @unchecked Sendable {
    let onChunk: (String, Bool) -> Void
    init(onChunk: @escaping (String, Bool) -> Void) {
        self.onChunk = onChunk
    }
}

private func streamBridge(chunk: UnsafePointer<CChar>?, isDone: Int32, context: UnsafeMutableRawPointer?) {
    guard let context else { return }
    let box = Unmanaged<StreamBox>.fromOpaque(context).takeUnretainedValue()
    let text = chunk.flatMap { String(cString: $0) } ?? ""
    box.onChunk(text, isDone != 0)
}

// MARK: - DesktopModel

@MainActor
private final class DesktopModel: ObservableObject {
    @Published var workspace: Workspace = .chat
    @Published var selectedToolTab: ToolTab = .tcpTest
    @Published var sessions: [ChatSession] = [] { didSet { persistSessions() } }
    @Published var activeSessionID: UUID? { didSet { persistActiveSession() } }
    @Published var draft = ""
    @Published var pendingAttachments: [PendingAttachment] = []
    @Published var attachmentError = ""
    @Published var isWorking = false
    @Published var connections: [SavedConnection] = [] { didSet { persistConnections() } }
    @Published var editingConnection: SavedConnection?
    @Published var connectionStatuses: [UUID: ConnectionTestStatus] = [:]
    @Published var documentsDirectory: String { didSet { defaults.set(documentsDirectory, forKey: "mikomai.desktop.mac.documentsDirectory") } }
    @Published var knowledgeDirectory: String { didSet { defaults.set(knowledgeDirectory, forKey: "mikomai.desktop.mac.knowledgeDirectory") } }
    @Published var modelPath: String { didSet { defaults.set(modelPath, forKey: "mikomai.desktop.mac.modelPath") } }
    @Published var modelStatus = "モデル未ロード"
    @Published var isLoadingModel = false
    @Published var isCancelling = false

    // Tools state
    @Published var tcpTestHost = ""
    @Published var tcpTestPort = "22"
    @Published var tcpTestTimeout = "2000"
    @Published var isTestingTcp = false
    @Published var tcpTestResult: String?
    @Published var tcpTestSuccess: Bool?
    @Published var recentTcpTests: [String] = []

    private let defaults = UserDefaults.standard
    private let sessionsKey = "mikomai.desktop.mac.sessions.v1"
    private let activeKey = "mikomai.desktop.mac.activeSession.v1"
    private let connectionsKey = "mikomai.desktop.mac.connections.v1"

    init() {
        let bundledDocuments = Bundle.main.resourceURL?.appendingPathComponent("nw-docs", isDirectory: true).path
        let defaultDocuments = bundledDocuments
            ?? URL(fileURLWithPath: FileManager.default.currentDirectoryPath).appendingPathComponent("nw-docs").path
        let defaultKnowledge = (FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first ?? FileManager.default.temporaryDirectory)
            .appendingPathComponent("MikomaiDesktopMac/knowledge", isDirectory: true).path
        documentsDirectory = defaults.string(forKey: "mikomai.desktop.mac.documentsDirectory")
            ?? ProcessInfo.processInfo.environment["MIKOMAI_DOCS_DIR"] ?? defaultDocuments
        knowledgeDirectory = defaults.string(forKey: "mikomai.desktop.mac.knowledgeDirectory")
            ?? ProcessInfo.processInfo.environment["MIKOMAI_KNOWLEDGE_DIR"] ?? defaultKnowledge
        modelPath = defaults.string(forKey: "mikomai.desktop.mac.modelPath") ?? ""
        if let data = defaults.data(forKey: sessionsKey),
           let decoded = try? JSONDecoder().decode([ChatSession].self, from: data) {
            sessions = decoded
        }
        if let value = defaults.string(forKey: activeKey), let id = UUID(uuidString: value), sessions.contains(where: { $0.id == id }) {
            activeSessionID = id
        } else {
            activeSessionID = sessions.first?.id
        }
        if let data = defaults.data(forKey: connectionsKey),
           let decoded = try? JSONDecoder().decode([SavedConnection].self, from: data) {
            connections = decoded
        }
        if sessions.isEmpty { createSession() }
        refreshModelStatus()
    }

    var activeSession: ChatSession? { sessions.first(where: { $0.id == activeSessionID }) }

    func createSession() {
        let session = ChatSession(title: "新しい会話")
        sessions.insert(session, at: 0)
        activeSessionID = session.id
        workspace = .chat
    }

    func select(_ id: UUID) { activeSessionID = id; workspace = .chat }

    func deleteSession(_ id: UUID) {
        sessions.removeAll { $0.id == id }
        if activeSessionID == id { activeSessionID = sessions.first?.id }
        if sessions.isEmpty { createSession() }
    }

    func renameSession(_ id: UUID, title: String) {
        guard let index = sessions.firstIndex(where: { $0.id == id }), !title.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return }
        sessions[index].title = title.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    // MARK: - Streaming Chat

    func send() {
        let prompt = draft.trimmingCharacters(in: .whitespacesAndNewlines)
        guard (!prompt.isEmpty || !pendingAttachments.isEmpty), !isWorking, let id = activeSessionID,
              let index = sessions.firstIndex(where: { $0.id == id }) else { return }
        let history = sessions[index].messages.suffix(12).map { message in
            "\(message.role == .user ? "ユーザー" : "MIKOMAI"): \(message.text)"
        }.joined(separator: "\n")
        let attachedNames = pendingAttachments.map(\.name)
        let userText = prompt.isEmpty ? "添付ファイルを確認してください。" : prompt
        let attachmentText = pendingAttachments.enumerated().map { offset, attachment in
            "[添付ファイル \(offset + 1): \(attachment.name)]\n\(attachment.text)"
        }.joined(separator: "\n\n")

        sessions[index].messages.append(ChatMessage(role: .user, text: userText, attachments: attachedNames))
        if sessions[index].messages.count == 1 { sessions[index].title = String(userText.prefix(36)) }

        let assistantMsg = ChatMessage(role: .assistant, text: "")
        let assistantID = assistantMsg.id
        sessions[index].messages.append(assistantMsg)
        sessions[index].updatedAt = Date()

        let documents = documentsDirectory
        let knowledge = knowledgeDirectory
        pendingAttachments = []
        attachmentError = ""
        draft = ""
        isWorking = true

        Task.detached(priority: .userInitiated) {
            let finalAnswer = Self.askRustStreaming(
                userText,
                history: history,
                documents: documents,
                knowledge: knowledge,
                attachments: attachmentText
            ) { chunk, _ in
                Task { @MainActor in
                    guard let sIdx = self.sessions.firstIndex(where: { $0.id == id }),
                          let mIdx = self.sessions[sIdx].messages.firstIndex(where: { $0.id == assistantID }) else { return }
                    self.sessions[sIdx].messages[mIdx].text += chunk
                    self.sessions[sIdx].updatedAt = Date()
                }
            }
            await MainActor.run {
                guard let sIdx = self.sessions.firstIndex(where: { $0.id == id }),
                      let mIdx = self.sessions[sIdx].messages.firstIndex(where: { $0.id == assistantID }) else {
                    self.isWorking = false
                    return
                }
                if self.sessions[sIdx].messages[mIdx].text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                    self.sessions[sIdx].messages[mIdx].text = finalAnswer
                }
                self.sessions[sIdx].updatedAt = Date()
                self.isWorking = false
                self.isCancelling = false
                self.persistSessions()
            }
        }
    }

    func selectAttachments() {
        guard !isWorking else { return }
        let panel = NSOpenPanel()
        panel.allowedContentTypes = [
            .plainText, .commaSeparatedText, .json, .yaml, .xml,
            UTType(filenameExtension: "md") ?? .plainText,
            UTType(filenameExtension: "log") ?? .plainText
        ]
        panel.canChooseFiles = true
        panel.canChooseDirectories = false
        panel.allowsMultipleSelection = true
        guard panel.runModal() == .OK else { return }

        let maxFileBytes = 64 * 1024
        let maxTotalBytes = 128 * 1024
        var loaded = pendingAttachments
        var totalBytes = loaded.reduce(0) { $0 + $1.byteCount }
        for url in panel.urls {
            guard !loaded.contains(where: { $0.name == url.lastPathComponent }) else { continue }
            let hasScope = url.startAccessingSecurityScopedResource()
            defer { if hasScope { url.stopAccessingSecurityScopedResource() } }
            do {
                let handle = try FileHandle(forReadingFrom: url)
                defer { try? handle.close() }
                let data = try handle.read(upToCount: maxFileBytes + 1) ?? Data()
                guard data.count <= maxFileBytes else { throw AttachmentReadError.tooLarge }
                guard totalBytes + data.count <= maxTotalBytes else { throw AttachmentReadError.totalTooLarge }
                guard let text = String(data: data, encoding: .utf8) else { throw AttachmentReadError.invalidEncoding }
                guard !text.unicodeScalars.contains(where: { $0.value == 0 }) else { throw AttachmentReadError.containsNull }
                loaded.append(PendingAttachment(name: url.lastPathComponent, text: text))
                totalBytes += data.count
            } catch {
                attachmentError = "\(url.lastPathComponent): \(error.localizedDescription)"
                pendingAttachments = loaded
                return
            }
        }
        pendingAttachments = loaded
        attachmentError = ""
    }

    func removeAttachment(_ id: UUID) { pendingAttachments.removeAll { $0.id == id } }

    func stop() {
        guard isWorking, !isCancelling else { return }
        isCancelling = true
        _ = Self.callRust { mikomai_model_cancel() }
    }

    // MARK: - Model Management

    func selectModel() {
        let panel = NSOpenPanel()
        if let gguf = UTType(filenameExtension: "gguf") { panel.allowedContentTypes = [gguf] }
        panel.canChooseFiles = true
        panel.canChooseDirectories = false
        panel.allowsMultipleSelection = false
        if panel.runModal() == .OK, let url = panel.url { modelPath = url.path }
    }

    func loadModel() {
        guard !modelPath.isEmpty, !isLoadingModel else { return }
        let path = modelPath
        isLoadingModel = true
        modelStatus = "モデルを読み込み中…"
        Task.detached(priority: .userInitiated) {
            let status = Self.callRust { path.withCString { mikomai_model_load($0) } }
            let loadedPath = Self.callRust { mikomai_model_status() }
            await MainActor.run {
                if status.hasPrefix("エラー") {
                    let current = loadedPath.isEmpty ? "" : " (現在: \(URL(fileURLWithPath: loadedPath).lastPathComponent))"
                    self.modelStatus = "\(status)\(current)"
                } else {
                    self.modelStatus = "読み込み済み: \(URL(fileURLWithPath: loadedPath).lastPathComponent)"
                }
                self.isLoadingModel = false
            }
        }
    }

    private func refreshModelStatus() {
        let status = Self.callRust { mikomai_model_status() }
        if !status.isEmpty { modelStatus = "読み込み済み: \(URL(fileURLWithPath: status).lastPathComponent)" }
    }

    // MARK: - Connections & Keychain

    func saveConnection(_ connection: SavedConnection, password: String? = nil, enablePassword: String? = nil) {
        guard connection.validationError == nil else { return }
        var updated = connection
        if let pwd = password, !pwd.isEmpty {
            KeychainHelper.save(key: "conn.\(connection.id.uuidString).password", value: pwd)
            updated.hasPassword = true
        } else if password != nil {
            KeychainHelper.delete(key: "conn.\(connection.id.uuidString).password")
            updated.hasPassword = false
        }

        if let enPwd = enablePassword, !enPwd.isEmpty {
            KeychainHelper.save(key: "conn.\(connection.id.uuidString).enable", value: enPwd)
            updated.hasEnablePassword = true
        } else if enablePassword != nil {
            KeychainHelper.delete(key: "conn.\(connection.id.uuidString).enable")
            updated.hasEnablePassword = false
        }

        if let index = connections.firstIndex(where: { $0.id == connection.id }) {
            connections[index] = updated
        } else {
            connections.append(updated)
        }
        editingConnection = nil
    }

    func deleteConnection(_ id: UUID) {
        connections.removeAll { $0.id == id }
        connectionStatuses.removeValue(forKey: id)
        KeychainHelper.delete(key: "conn.\(id.uuidString).password")
        KeychainHelper.delete(key: "conn.\(id.uuidString).enable")
    }

    func testConnection(_ connection: SavedConnection) {
        let host = connection.host
        let port = UInt16(connection.port) ?? 22
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

    func importTauriDevices(_ devices: [TauriDeviceSummary]) -> (imported: Int, skipped: Int) {
        var imported = 0
        var skipped = 0
        for device in devices {
            if let sourceID = device.id, connections.contains(where: { $0.sourceID == sourceID }) {
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
                deviceType: device.deviceType ?? device.connectionType ?? "不明"
            )
            guard connection.validationError == nil else { skipped += 1; continue }
            connections.append(connection)
            imported += 1
        }
        return (imported, skipped)
    }

    private func persistSessions() {
        guard let data = try? JSONEncoder().encode(sessions) else { return }
        defaults.set(data, forKey: sessionsKey)
    }

    private func persistActiveSession() { defaults.set(activeSessionID?.uuidString, forKey: activeKey) }
    private func persistConnections() {
        guard let data = try? JSONEncoder().encode(connections) else { return }
        defaults.set(data, forKey: connectionsKey)
    }

    // MARK: - Rust FFI Calls

    private nonisolated static func askRustStreaming(
        _ prompt: String,
        history: String,
        documents: String,
        knowledge: String,
        attachments: String,
        onChunk: @escaping (String, Bool) -> Void
    ) -> String {
        let box = StreamBox(onChunk: onChunk)
        let context = Unmanaged.passUnretained(box).toOpaque()
        let response = prompt.withCString { message in
            history.withCString { historyText in
                documents.withCString { documentsPath in
                    knowledge.withCString { knowledgePath in
                        attachments.withCString { attachmentText in
                            mikomai_assistant_chat_streaming(
                                message,
                                historyText,
                                documentsPath,
                                knowledgePath,
                                attachmentText,
                                streamBridge,
                                context
                            )
                        }
                    }
                }
            }
        }
        defer { mikomai_result_free(response) }
        guard let message = response.message else { return "Rust 側から応答がありませんでした。" }
        let text = String(cString: message)
        return response.status == 0 ? text : "エラー: \(text)"
    }

    private nonisolated static func testTCP(host: String, port: UInt16, timeoutMs: UInt32) -> (success: Bool, message: String, latencyMs: Int?) {
        let response = host.withCString { cHost in
            mikomai_test_tcp_connection(cHost, port, timeoutMs)
        }
        defer { mikomai_result_free(response) }
        guard let msg = response.message else { return (false, "応答がありませんでした", nil) }
        let text = String(cString: msg)
        let latency: Int? = {
            if let msRange = text.range(of: "ms") {
                let prefix = text[..<msRange.lowerBound].trimmingCharacters(in: .whitespaces)
                if let lastSep = prefix.lastIndex(where: { $0 == " " || $0 == "," }) {
                    let numPart = prefix[prefix.index(after: lastSep)...]
                    return Int(numPart)
                }
            }
            return nil
        }()
        return (response.status == 0, text, latency)
    }

    private nonisolated static func callRust(_ call: () -> MikomaiResult) -> String {
        let response = call()
        defer { mikomai_result_free(response) }
        guard let message = response.message else { return "応答がありませんでした。" }
        let text = String(cString: message)
        return response.status == 0 ? text : "エラー: \(text)"
    }
}

private enum AttachmentReadError: LocalizedError {
    case tooLarge
    case totalTooLarge
    case invalidEncoding
    case containsNull

    var errorDescription: String? {
        switch self {
        case .tooLarge: "ファイルは64 KiB以下にしてください。"
        case .totalTooLarge: "添付ファイルの合計は128 KiB以下にしてください。"
        case .invalidEncoding: "UTF-8テキストではありません。"
        case .containsNull: "NUL文字を含むファイルは添付できません。"
        }
    }
}

// MARK: - Diagnostics Runners

@MainActor
private final class DiagnosticsRunner: ObservableObject {
    @Published var output = ""
    @Published var isRunning = false
    private var process: Process?

    func run(command: String, arguments: [String]) {
        stop()
        isRunning = true
        output = "$ \(command) \(arguments.joined(separator: " "))\n"

        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: command)
        proc.arguments = arguments

        let pipe = Pipe()
        proc.standardOutput = pipe
        proc.standardError = pipe

        pipe.fileHandleForReading.readabilityHandler = { [weak self] handle in
            let data = handle.availableData
            guard !data.isEmpty, let str = String(data: data, encoding: .utf8) else { return }
            Task { @MainActor in
                self?.output += str
            }
        }

        proc.terminationHandler = { [weak self] _ in
            Task { @MainActor in
                self?.isRunning = false
                self?.output += "\n[完了]\n"
                self?.process = nil
            }
        }

        self.process = proc
        do {
            try proc.run()
        } catch {
            output += "エラー: コマンド起動に失敗しました: \(error.localizedDescription)\n"
            isRunning = false
        }
    }

    func stop() {
        if let proc = process, proc.isRunning {
            proc.terminate()
            output += "\n[停止しました]\n"
        }
        process = nil
        isRunning = false
    }

    func clear() {
        output = ""
    }
}

private enum NetworkInspector {
    static func fetchArpTable() -> [ArpRecord] {
        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: "/usr/sbin/arp")
        proc.arguments = ["-an"]
        let pipe = Pipe()
        proc.standardOutput = pipe
        proc.standardError = FileHandle.nullDevice
        do {
            try proc.run()
            proc.waitUntilExit()
            let data = pipe.fileHandleForReading.readDataToEndOfFile()
            let text = String(data: data, encoding: .utf8) ?? ""
            return parseArp(text)
        } catch {
            return []
        }
    }

    private static func parseArp(_ text: String) -> [ArpRecord] {
        var records: [ArpRecord] = []
        for line in text.components(separatedBy: .newlines) {
            let trimmed = line.trimmingCharacters(in: .whitespaces)
            guard !trimmed.isEmpty else { continue }
            guard let openParen = trimmed.firstIndex(of: "("),
                  let closeParen = trimmed.firstIndex(of: ")"),
                  openParen < closeParen else { continue }
            let ip = String(trimmed[trimmed.index(after: openParen)..<closeParen])
            guard let atRange = trimmed.range(of: " at ") else { continue }
            let afterAt = trimmed[atRange.upperBound...]
            guard let onRange = afterAt.range(of: " on ") else { continue }
            let mac = String(afterAt[..<onRange.lowerBound]).trimmingCharacters(in: .whitespaces)
            let afterOn = afterAt[onRange.upperBound...]
            let iface = afterOn.components(separatedBy: .whitespaces).first ?? ""
            let isPermanent = trimmed.contains("permanent")
            let isIncomplete = mac.contains("incomplete")
            records.append(ArpRecord(ip: ip, mac: mac, interface: iface, isPermanent: isPermanent, isIncomplete: isIncomplete))
        }
        return records
    }

    static func fetchRoutingTable() -> [RouteRecord] {
        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: "/usr/sbin/netstat")
        proc.arguments = ["-rn", "-f", "inet"]
        let pipe = Pipe()
        proc.standardOutput = pipe
        proc.standardError = FileHandle.nullDevice
        do {
            try proc.run()
            proc.waitUntilExit()
            let data = pipe.fileHandleForReading.readDataToEndOfFile()
            let text = String(data: data, encoding: .utf8) ?? ""
            return parseRoutes(text)
        } catch {
            return []
        }
    }

    private static func parseRoutes(_ text: String) -> [RouteRecord] {
        var records: [RouteRecord] = []
        var inTable = false
        for line in text.components(separatedBy: .newlines) {
            let trimmed = line.trimmingCharacters(in: .whitespaces)
            guard !trimmed.isEmpty else { continue }
            if trimmed.hasPrefix("Destination") {
                inTable = true
                continue
            }
            guard inTable else { continue }
            let parts = trimmed.components(separatedBy: .whitespaces).filter { !$0.isEmpty }
            if parts.count >= 4 {
                let dest = parts[0]
                let gateway = parts[1]
                let flags = parts[2]
                let iface = parts[3]
                records.append(RouteRecord(destination: dest, gateway: gateway, flags: flags, interface: iface))
            }
        }
        return records
    }
}

// MARK: - Main Desktop Window

private struct DesktopWindow: View {
    @ObservedObject var model: DesktopModel

    var body: some View {
        HStack(spacing: 0) {
            activityBar
            if model.workspace == .chat { historySidebar }
            VStack(spacing: 0) {
                header
                Group {
                    switch model.workspace {
                    case .chat: chatWorkspace
                    case .connections: ConnectionsWorkspace(model: model)
                    case .tools: NetworkToolsWorkspace(model: model)
                    case .settings: SettingsWorkspace(model: model)
                    }
                }
                statusBar
            }
            .background(Color(nsColor: .windowBackgroundColor))
        }
        .background(Color(nsColor: .underPageBackgroundColor))
        .sheet(item: $model.editingConnection) { connection in
            ConnectionEditor(connection: connection) { saved, pwd, enPwd in
                model.saveConnection(saved, password: pwd, enablePassword: enPwd)
            }
        }
    }

    private var activityBar: some View {
        VStack(spacing: 8) {
            Image(systemName: "point.3.connected.trianglepath.dotted")
                .font(.system(size: 19, weight: .semibold)).foregroundStyle(Color.accentColor)
                .frame(width: 34, height: 34).padding(.bottom, 8)
            ForEach(Workspace.allCases) { item in
                Button { model.workspace = item } label: {
                    Image(systemName: item.icon).font(.system(size: 16, weight: .medium))
                        .foregroundStyle(model.workspace == item ? .primary : .secondary)
                        .frame(width: 34, height: 34)
                        .background(model.workspace == item ? Color(nsColor: .selectedContentBackgroundColor).opacity(0.16) : .clear, in: RoundedRectangle(cornerRadius: 6))
                }
                .buttonStyle(.plain).help(item.rawValue)
            }
            Spacer()
        }
        .padding(.top, 12).frame(width: 50)
        .background(Color(nsColor: .controlBackgroundColor))
        .overlay(alignment: .trailing) { Divider() }
    }

    private var historySidebar: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack {
                Text("会話").font(.system(size: 12, weight: .semibold)).foregroundStyle(.secondary)
                Spacer()
                Button { model.createSession() } label: { Image(systemName: "square.and.pencil") }
                    .buttonStyle(.plain).help("新しい会話")
            }
            .padding(.horizontal, 14).padding(.vertical, 12)
            Divider()
            ScrollView {
                VStack(spacing: 2) {
                    ForEach(model.sessions) { session in
                        SessionRow(session: session, isSelected: session.id == model.activeSessionID,
                                   onSelect: { model.select(session.id) },
                                   onRename: { model.renameSession(session.id, title: $0) },
                                   onDelete: { model.deleteSession(session.id) })
                    }
                }.padding(8)
            }
            Spacer(minLength: 0)
            Divider()
            HStack(spacing: 8) {
                Image(systemName: "books.vertical").foregroundStyle(.secondary)
                VStack(alignment: .leading, spacing: 2) {
                    Text("ローカルナレッジ").font(.system(size: 11, weight: .medium))
                    Text(URL(fileURLWithPath: model.documentsDirectory).lastPathComponent).font(.system(size: 10, design: .monospaced)).foregroundStyle(.secondary)
                }
            }.padding(12)
        }
        .frame(width: 248)
        .background(Color(nsColor: .controlBackgroundColor).opacity(0.7))
        .overlay(alignment: .trailing) { Divider() }
    }

    private var header: some View {
        HStack {
            VStack(alignment: .leading, spacing: 2) {
                Text(model.workspace == .chat ? (model.activeSession?.title ?? "mikomai") : model.workspace.rawValue)
                    .font(.system(size: 14, weight: .semibold))
                Text(model.workspace == .chat ? "ネットワークアシスタント" : "mikomai desktop")
                    .font(.system(size: 11)).foregroundStyle(.secondary)
            }
            Spacer()
            if model.workspace == .chat {
                Circle().fill(.green).frame(width: 7, height: 7)
                Text("ローカルナレッジ").font(.system(size: 11)).foregroundStyle(.secondary)
            }
        }
        .padding(.horizontal, 20).padding(.vertical, 12)
        .background(Color(nsColor: .windowBackgroundColor))
        .overlay(alignment: .bottom) { Divider() }
    }

    private var statusBar: some View {
        HStack(spacing: 14) {
            HStack(spacing: 5) {
                Circle()
                    .fill(model.modelStatus.hasPrefix("読み込み済み") ? Color.green : Color.orange)
                    .frame(width: 7, height: 7)
                Text(model.modelStatus)
                    .font(.system(size: 11))
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
            }

            Divider().frame(height: 12)

            HStack(spacing: 4) {
                Image(systemName: "books.vertical").font(.system(size: 10)).foregroundStyle(.secondary)
                Text(URL(fileURLWithPath: model.documentsDirectory).lastPathComponent)
                    .font(.system(size: 10, design: .monospaced))
                    .foregroundStyle(.secondary)
            }

            Spacer()

            HStack(spacing: 4) {
                Image(systemName: "point.3.connected.trianglepath.dotted").font(.system(size: 10)).foregroundStyle(.secondary)
                Text("登録機器: \(model.connections.count)台")
                    .font(.system(size: 11))
                    .foregroundStyle(.secondary)
            }

            if model.workspace == .chat, let count = model.activeSession?.messages.count {
                Divider().frame(height: 12)
                Text("メッセージ: \(count)件")
                    .font(.system(size: 11))
                    .foregroundStyle(.secondary)
            }
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 6)
        .background(Color(nsColor: .controlBackgroundColor))
        .overlay(alignment: .top) { Divider() }
    }

    private var chatWorkspace: some View {
        VStack(spacing: 0) {
            ScrollViewReader { proxy in
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 18) {
                        if let session = model.activeSession, session.messages.isEmpty { emptyState }
                        if let session = model.activeSession {
                            ForEach(session.messages) { message in MessageRow(message: message).id(message.id) }
                        }
                        if model.isWorking {
                            HStack(spacing: 8) {
                                ProgressView().controlSize(.small)
                                Text(model.isCancelling ? "生成を停止しています…" : "資料を検索して回答を生成しています…")
                                    .font(.system(size: 12)).foregroundStyle(.secondary)
                            }
                            .padding(.leading, 42)
                        }
                    }
                    .frame(maxWidth: 760).frame(maxWidth: .infinity).padding(.horizontal, 24).padding(.vertical, 24)
                }
                .onChange(of: model.activeSession?.messages.last?.text ?? "") { _ in
                    if let message = model.activeSession?.messages.last { proxy.scrollTo(message.id, anchor: .bottom) }
                }
                .onChange(of: model.activeSession?.messages.count ?? 0) { _ in
                    if let message = model.activeSession?.messages.last { proxy.scrollTo(message.id, anchor: .bottom) }
                }
            }
            composer
        }
    }

    private var emptyState: some View {
        VStack(alignment: .leading, spacing: 12) {
            Image(systemName: "network").font(.system(size: 23)).foregroundStyle(Color.accentColor)
            Text("ネットワークの資料を探す").font(.system(size: 21, weight: .semibold))
            Text("機器の設定やトラブルシュートについて質問してください。ストリーミング応答とローカルRAGに対応しています。").font(.system(size: 13)).foregroundStyle(.secondary)
            HStack(spacing: 8) {
                suggestion("F220 の VLAN 設定")
                suggestion("Cisco の MAC アドレス確認")
            }.padding(.top, 4)
        }.frame(maxWidth: .infinity, alignment: .leading).padding(.top, 64)
    }

    private func suggestion(_ text: String) -> some View {
        Button(text) { model.draft = text }.buttonStyle(.bordered).controlSize(.small)
    }

    private var composer: some View {
        VStack(alignment: .leading, spacing: 6) {
            if !model.pendingAttachments.isEmpty {
                ScrollView(.horizontal) {
                    HStack(spacing: 6) {
                        ForEach(model.pendingAttachments) { attachment in
                            HStack(spacing: 5) {
                                Image(systemName: "doc.text")
                                Text(attachment.name).lineLimit(1)
                                Button { model.removeAttachment(attachment.id) } label: {
                                    Image(systemName: "xmark").font(.system(size: 9, weight: .semibold))
                                }.buttonStyle(.plain).help("添付を削除")
                            }
                            .font(.system(size: 11)).padding(.horizontal, 8).padding(.vertical, 5)
                            .background(Color(nsColor: .controlBackgroundColor), in: RoundedRectangle(cornerRadius: 5))
                        }
                    }
                }.scrollIndicators(.hidden)
            }
            if !model.attachmentError.isEmpty {
                Text(model.attachmentError).font(.system(size: 11)).foregroundStyle(.red)
            }
            HStack(alignment: .bottom, spacing: 10) {
                Button(action: model.selectAttachments) {
                    Image(systemName: "paperclip").font(.system(size: 13)).frame(width: 28, height: 28)
                }
                .buttonStyle(.bordered).controlSize(.small).disabled(model.isWorking).help("テキストファイルを添付")
                TextField("質問を入力…", text: $model.draft, axis: .vertical)
                    .textFieldStyle(.plain).lineLimit(1...5).onSubmit(model.send).disabled(model.isWorking)
                if model.isWorking {
                    Button(action: model.stop) { Image(systemName: "stop.fill").font(.system(size: 10, weight: .semibold)).frame(width: 28, height: 28) }
                        .buttonStyle(.bordered).controlSize(.small).disabled(model.isCancelling).help("生成を停止")
                } else {
                    Button(action: model.send) { Image(systemName: "arrow.up").font(.system(size: 12, weight: .semibold)).frame(width: 28, height: 28) }
                        .buttonStyle(.borderedProminent).controlSize(.small).keyboardShortcut(.return, modifiers: [.command])
                        .disabled(model.draft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty && model.pendingAttachments.isEmpty).help("送信")
                }
            }
        }
        .padding(10).background(Color(nsColor: .textBackgroundColor)).clipShape(RoundedRectangle(cornerRadius: 7))
        .overlay(RoundedRectangle(cornerRadius: 7).stroke(Color(nsColor: .separatorColor), lineWidth: 0.7))
        .frame(maxWidth: 760).padding(.horizontal, 22).padding(.top, 10).padding(.bottom, 14)
        .frame(maxWidth: .infinity).background(Color(nsColor: .windowBackgroundColor))
    }
}

// MARK: - Session & Message Rows

private struct SessionRow: View {
    let session: ChatSession
    let isSelected: Bool
    let onSelect: () -> Void
    let onRename: (String) -> Void
    let onDelete: () -> Void
    @State private var isRenaming = false
    @State private var title = ""

    var body: some View {
        HStack(spacing: 7) {
            Image(systemName: "bubble.left").font(.system(size: 11)).foregroundStyle(.secondary)
            if isRenaming {
                TextField("会話名", text: $title, onCommit: { onRename(title); isRenaming = false })
                    .textFieldStyle(.plain).font(.system(size: 12))
            } else {
                Button(action: onSelect) {
                    Text(session.title).font(.system(size: 12)).lineLimit(1).frame(maxWidth: .infinity, alignment: .leading)
                }.buttonStyle(.plain)
            }
            Menu {
                Button("名前を変更") { title = session.title; isRenaming = true }
                Button("削除", role: .destructive, action: onDelete)
            } label: { Image(systemName: "ellipsis").font(.system(size: 12)).frame(width: 20, height: 22) }
                .menuStyle(.borderlessButton).frame(width: 20)
        }
        .padding(.horizontal, 8).padding(.vertical, 6).background(isSelected ? Color(nsColor: .selectedContentBackgroundColor).opacity(0.16) : .clear, in: RoundedRectangle(cornerRadius: 5))
    }
}

private struct MessageRow: View {
    let message: ChatMessage
    var body: some View {
        HStack(alignment: .top, spacing: 10) {
            Image(systemName: message.role == .user ? "person.fill" : "point.3.connected.trianglepath.dotted")
                .font(.system(size: 12, weight: .semibold)).foregroundStyle(message.role == .user ? Color.secondary : Color.accentColor)
                .frame(width: 27, height: 27).background(Color(nsColor: .controlBackgroundColor), in: RoundedRectangle(cornerRadius: 6))
            Group {
                if message.role == .assistant {
                    MarkdownMessage(text: message.text)
                } else {
                    VStack(alignment: .leading, spacing: 6) {
                        Text(message.text).font(.system(size: 13)).textSelection(.enabled)
                        if !message.attachments.isEmpty {
                            ForEach(message.attachments, id: \.self) { name in
                                Label(name, systemImage: "doc.text").font(.system(size: 11)).foregroundStyle(.secondary)
                            }
                        }
                    }
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading).padding(.top, 4)
        }.frame(maxWidth: .infinity, alignment: .leading)
    }
}

// MARK: - Markdown Message Parser & View

private struct MarkdownBlock: Identifiable {
    enum Kind {
        case heading(Int, String)
        case paragraph(String)
        case code(String, String)
        case bullet(String)
        case quote(String)
        case separator
    }
    let id = UUID()
    let kind: Kind
}

private struct MarkdownMessage: View {
    let text: String
    private var blocks: [MarkdownBlock] { Self.parse(text) }

    var body: some View {
        VStack(alignment: .leading, spacing: 9) {
            ForEach(blocks) { block in
                switch block.kind {
                case let .heading(level, content):
                    inline(content)
                        .font(.system(size: level == 1 ? 21 : level == 2 ? 18 : 15, weight: .semibold))
                        .padding(.top, level <= 2 ? 5 : 2)
                case let .paragraph(content):
                    inline(content).font(.system(size: 13)).lineSpacing(3)
                case let .code(language, content):
                    VStack(alignment: .leading, spacing: 6) {
                        HStack {
                            if !language.isEmpty { Text(language).font(.system(size: 10, weight: .medium)).foregroundStyle(.secondary) }
                            Spacer()
                            Button("コピー") {
                                NSPasteboard.general.clearContents()
                                NSPasteboard.general.setString(content, forType: .string)
                            }
                            .buttonStyle(.borderless)
                            .font(.system(size: 10))
                            .foregroundStyle(.secondary)
                        }
                        Text(content).font(.system(size: 12, design: .monospaced)).textSelection(.enabled)
                            .frame(maxWidth: .infinity, alignment: .leading)
                    }
                    .padding(10).background(Color(nsColor: .controlBackgroundColor), in: RoundedRectangle(cornerRadius: 6))
                case let .bullet(content):
                    HStack(alignment: .firstTextBaseline, spacing: 8) {
                        Text("•").foregroundStyle(.secondary)
                        inline(content).font(.system(size: 13)).lineSpacing(3)
                    }.padding(.leading, 4)
                case let .quote(content):
                    inline(content).font(.system(size: 13)).foregroundStyle(.secondary)
                        .padding(.leading, 10).overlay(alignment: .leading) { Rectangle().fill(Color.accentColor.opacity(0.45)).frame(width: 2) }
                case .separator:
                    Divider()
                }
            }
        }
        .textSelection(.enabled)
    }

    private func inline(_ source: String) -> Text {
        if let attributed = try? AttributedString(markdown: source, options: .init(interpretedSyntax: .inlineOnlyPreservingWhitespace)) {
            return Text(attributed)
        }
        return Text(source)
    }

    private static func parse(_ source: String) -> [MarkdownBlock] {
        let lines = source.components(separatedBy: .newlines)
        var result: [MarkdownBlock] = []
        var paragraph: [String] = []
        var index = 0

        func flushParagraph() {
            guard !paragraph.isEmpty else { return }
            result.append(MarkdownBlock(kind: .paragraph(paragraph.joined(separator: " "))))
            paragraph.removeAll(keepingCapacity: true)
        }

        while index < lines.count {
            let line = lines[index]
            let trimmed = line.trimmingCharacters(in: .whitespaces)
            if trimmed.isEmpty { flushParagraph(); index += 1; continue }
            if trimmed.hasPrefix("```") {
                flushParagraph()
                let language = String(trimmed.dropFirst(3)).trimmingCharacters(in: .whitespaces)
                index += 1
                var code: [String] = []
                while index < lines.count && !lines[index].trimmingCharacters(in: .whitespaces).hasPrefix("```") {
                    code.append(lines[index]); index += 1
                }
                if index < lines.count { index += 1 }
                result.append(MarkdownBlock(kind: .code(language, code.joined(separator: "\n"))))
                continue
            }
            if ["---", "***", "___"].contains(trimmed) {
                flushParagraph(); result.append(MarkdownBlock(kind: .separator)); index += 1; continue
            }
            let hashCount = trimmed.prefix(while: { $0 == "#" }).count
            if (1...6).contains(hashCount), trimmed.dropFirst(hashCount).first == " " {
                flushParagraph()
                result.append(MarkdownBlock(kind: .heading(hashCount, String(trimmed.dropFirst(hashCount)).trimmingCharacters(in: .whitespaces))))
                index += 1
                continue
            }
            if trimmed.hasPrefix("> ") {
                flushParagraph(); result.append(MarkdownBlock(kind: .quote(String(trimmed.dropFirst(2))))); index += 1; continue
            }
            if ["- ", "* ", "+ "].contains(where: { trimmed.hasPrefix($0) }) {
                flushParagraph(); result.append(MarkdownBlock(kind: .bullet(String(trimmed.dropFirst(2))))); index += 1; continue
            }
            paragraph.append(trimmed)
            index += 1
        }
        flushParagraph()
        return result
    }
}

// MARK: - Connections Workspace

private struct ConnectionsWorkspace: View {
    @ObservedObject var model: DesktopModel
    @State private var csvAlert = ""
    @State private var showsCSVAlert = false

    var body: some View {
        VStack(spacing: 0) {
            HStack {
                Text("機器情報一覧").font(.system(size: 13, weight: .semibold))
                Spacer()
                Button("Tauri から取り込む") { importTauriRegistry() }
                    .buttonStyle(.bordered).controlSize(.small)
                Button { model.editingConnection = SavedConnection(name: "", host: "") } label: { Label("機器を追加", systemImage: "plus") }
                    .buttonStyle(.borderedProminent).controlSize(.small)
            }.padding(16)
            Divider()
            if model.connections.isEmpty {
                VStack(spacing: 9) {
                    Image(systemName: "point.3.connected.trianglepath.dotted").font(.system(size: 24)).foregroundStyle(.secondary)
                    Text("登録した機器はありません").font(.system(size: 14, weight: .semibold))
                    Text("ネットワーク機器の接続情報を登録できます。Keychain による資格情報の安全な保存、Ping や接続テストを実行できます。").font(.system(size: 12)).foregroundStyle(.secondary)
                }.frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                Table(model.connections) {
                    TableColumn("名前", value: \.name)
                    TableColumn("登録元") { connection in
                        Text(connection.sourceID == nil ? "Mac 内" : "Tauri")
                            .font(.system(size: 11)).foregroundStyle(.secondary)
                    }.width(62)
                    TableColumn("ホスト", value: \.host)
                    TableColumn("ポート", value: \.port).width(50)
                    TableColumn("ユーザー", value: \.username)
                    TableColumn("機器タイプ", value: \.deviceType)
                    TableColumn("資格情報") { connection in
                        if connection.hasPassword {
                            Label("Key", systemImage: "key.fill").font(.system(size: 11)).foregroundStyle(.green)
                        } else {
                            Text("未設定").font(.system(size: 11)).foregroundStyle(.secondary)
                        }
                    }.width(65)
                    TableColumn("ステータス") { connection in
                        if let status = model.connectionStatuses[connection.id] {
                            HStack(spacing: 4) {
                                Circle().fill(status.success ? Color.green : Color.red).frame(width: 7, height: 7)
                                if let lat = status.latencyMs {
                                    Text("\(lat) ms").font(.system(size: 11))
                                } else {
                                    Text(status.success ? "OK" : "NG").font(.system(size: 11))
                                }
                            }
                        } else {
                            Text("未テスト").font(.system(size: 11)).foregroundStyle(.secondary)
                        }
                    }.width(75)
                    TableColumn("操作") { connection in
                        HStack(spacing: 6) {
                            Button {
                                model.testConnection(connection)
                            } label: {
                                Image(systemName: "bolt.fill")
                            }
                            .help("接続テスト")

                            Button {
                                model.tcpTestHost = connection.host
                                model.workspace = .tools
                                model.selectedToolTab = .ping
                            } label: {
                                Image(systemName: "antenna.radiowaves.left.and.right")
                            }
                            .help("Ping を実行")

                            Button {
                                model.editingConnection = connection
                            } label: {
                                Image(systemName: "pencil")
                            }
                            .help("編集")

                            Button(role: .destructive) {
                                model.deleteConnection(connection.id)
                            } label: {
                                Image(systemName: "trash")
                            }
                            .help("削除")
                        }
                        .buttonStyle(.borderless)
                    }.width(115)
                }
            }
            Spacer(minLength: 0)
            HStack {
                Text("資格情報は macOS Keychain に暗号化保存されます。CSV 形式での入出力や Tauri 版のメタデータ取り込みに対応しています。")
                    .font(.system(size: 11)).foregroundStyle(.secondary)
                Spacer()
                Button("CSV を読み込む") { importCSV() }
                Button("CSV を書き出す") { exportCSV() }.disabled(model.connections.isEmpty)
            }.padding(12).background(Color(nsColor: .controlBackgroundColor).opacity(0.5))
        }
        .alert("機器情報", isPresented: $showsCSVAlert) { Button("OK", role: .cancel) {} } message: { Text(csvAlert) }
    }

    private func importTauriRegistry() {
        let panel = NSOpenPanel()
        panel.allowedContentTypes = [.json]
        panel.canChooseFiles = true
        panel.canChooseDirectories = false
        panel.allowsMultipleSelection = false
        guard panel.runModal() == .OK, let url = panel.url else { return }
        let scoped = url.startAccessingSecurityScopedResource()
        defer { if scoped { url.stopAccessingSecurityScopedResource() } }
        let json = url.path.withCString { path in
            let response = mikomai_device_registry_read(path)
            defer { mikomai_result_free(response) }
            guard let message = response.message else { return "エラー: 機器情報を読み込めませんでした。" }
            let text = String(cString: message)
            return response.status == 0 ? text : "エラー: \(text)"
        }
        guard !json.hasPrefix("エラー:") else {
            presentCSVMessage(json)
            return
        }
        guard let devices = try? JSONDecoder().decode([TauriDeviceSummary].self, from: Data(json.utf8)) else {
            presentCSVMessage("Tauri の機器情報 JSON を読み取れませんでした。元ファイルは変更していません。")
            return
        }
        let result = model.importTauriDevices(devices)
        let missingIDs = devices.filter { $0.id == nil }.count
        var note = "\(result.imported) 件を追加し、\(result.skipped) 件をスキップしました。元ファイルは変更していません。"
        if missingIDs > 0 { note += " IDのない行は重複判定せず追加しました。" }
        presentCSVMessage(note)
    }

    private func exportCSV() {
        let panel = NSSavePanel(); panel.nameFieldStringValue = "connections.csv"
        guard panel.runModal() == .OK, let url = panel.url else { return }
        if let invalid = model.connections.first(where: { $0.validationError != nil }) {
            presentCSVMessage("\(invalid.name) のホスト名が Tauri CSV 形式の制約に合いません。機器情報を編集してください。")
            return
        }
        let rows = ["id,status,hostname,ip,port,type,lastConnected,deviceType,vendorType,username"] + model.connections.map {
            ["", "offline", $0.name, $0.host, $0.port, "SSH", "Never", $0.deviceType, "", $0.username].map(csvEscape).joined(separator: ",")
        }
        do {
            try rows.joined(separator: "\n").write(to: url, atomically: true, encoding: .utf8)
        } catch {
            presentCSVMessage("CSV ファイルを書き込めませんでした。\(error.localizedDescription)")
        }
    }

    private func importCSV() {
        let panel = NSOpenPanel()
        panel.allowedContentTypes = [.commaSeparatedText]
        panel.allowsMultipleSelection = false
        guard panel.runModal() == .OK, let url = panel.url,
              let content = try? String(contentsOf: url, encoding: .utf8) else { return }
        guard let rows = parseCSV(content), let firstHeader = rows.first else {
            presentCSVMessage("CSV の引用符または行形式を確認してください。")
            return
        }
        var header = firstHeader.map { $0.trimmingCharacters(in: .whitespacesAndNewlines).lowercased() }
        if let first = header.first { header[0] = first.trimmingCharacters(in: CharacterSet(charactersIn: "\u{feff}")) }
        var columns: [String: Int] = [:]
        for (index, key) in header.enumerated() where columns[key] == nil { columns[key] = index }
        func value(_ row: [String], _ key: String, fallback: String = "") -> String {
            guard let index = columns[key], row.indices.contains(index) else { return fallback }
            return row[index].trimmingCharacters(in: .whitespacesAndNewlines)
        }
        var importedCount = 0
        var skippedCount = 0
        for row in rows.dropFirst() where !row.allSatisfy({ $0.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty }) {
            guard row.count == header.count else { skippedCount += 1; continue }
            let name = value(row, "name", fallback: value(row, "hostname"))
            let host = value(row, "host", fallback: value(row, "ip"))
            guard !name.isEmpty, !host.isEmpty else { skippedCount += 1; continue }
            let connection = SavedConnection(name: name, host: host, port: value(row, "port", fallback: "22"), username: value(row, "username"), deviceType: value(row, "devicetype", fallback: "Cisco IOS"))
            guard connection.validationError == nil else { skippedCount += 1; continue }
            model.saveConnection(connection)
            importedCount += 1
        }
        presentCSVMessage("\(importedCount) 件を読み込みました。\(skippedCount) 件は形式が合わないためスキップしました。")
    }

    private func parseCSV(_ input: String) -> [[String]]? {
        var rows: [[String]] = []
        var row: [String] = []
        var field = ""
        var quoted = false
        var closedQuote = false
        let characters = Array(input)
        var index = 0
        while index < characters.count {
            let character = characters[index]
            if quoted {
                if character == "\"" {
                    if index + 1 < characters.count && characters[index + 1] == "\"" { field.append("\""); index += 1 }
                    else { quoted = false; closedQuote = true }
                } else { field.append(character) }
            } else if character == "\"" {
                guard field.isEmpty && !closedQuote else { return nil }
                quoted = true
            }
            else if character == "," { row.append(field); field = ""; closedQuote = false }
            else if character == "\n" || character == "\r" {
                if character == "\r", index + 1 < characters.count, characters[index + 1] == "\n" { index += 1 }
                row.append(field); rows.append(row); row = []; field = ""; closedQuote = false
            } else {
                guard !closedQuote else { return nil }
                field.append(character)
            }
            index += 1
        }
        guard !quoted else { return nil }
        if !field.isEmpty || !row.isEmpty { row.append(field); rows.append(row) }
        return rows
    }

    private func presentCSVMessage(_ message: String) {
        csvAlert = message
        showsCSVAlert = true
    }

    private func csvEscape(_ value: String) -> String { "\"\(value.replacingOccurrences(of: "\"", with: "\"\""))\"" }
}

// MARK: - Connection Editor with Keychain

private struct ConnectionEditor: View {
    @Environment(\.dismiss) private var dismiss
    @State var connection: SavedConnection
    @State private var password = ""
    @State private var enablePassword = ""
    let onSave: (SavedConnection, String?, String?) -> Void
    private let deviceTypes = ["Cisco IOS", "Cisco NX-OS", "Juniper JunOS", "Arista EOS", "Other"]

    init(connection: SavedConnection, onSave: @escaping (SavedConnection, String?, String?) -> Void) {
        self._connection = State(initialValue: connection)
        self.onSave = onSave
        let existingPwd = KeychainHelper.load(key: "conn.\(connection.id.uuidString).password") ?? ""
        let existingEn = KeychainHelper.load(key: "conn.\(connection.id.uuidString).enable") ?? ""
        self._password = State(initialValue: existingPwd)
        self._enablePassword = State(initialValue: existingEn)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text(connection.name.isEmpty ? "機器を追加" : "機器情報を編集").font(.system(size: 16, weight: .semibold))
            Form {
                TextField("名前", text: $connection.name)
                TextField("ホスト名または IP", text: $connection.host)
                TextField("ポート", text: $connection.port)
                TextField("ユーザー名", text: $connection.username)
                Picker("機器タイプ", selection: $connection.deviceType) { ForEach(deviceTypes, id: \.self) { Text($0) } }

                Section("資格情報 (Keychain)") {
                    SecureField("パスワード", text: $password)
                    SecureField("Enable パスワード", text: $enablePassword)
                    Text("パスワードは macOS Keychain に暗号化されて安全に保管されます。平文ファイルには保存されません。")
                        .font(.system(size: 10)).foregroundStyle(.secondary)
                }
            }.formStyle(.grouped)
            HStack {
                Spacer()
                Button("キャンセル") { dismiss() }
                Button("保存") {
                    onSave(connection, password, enablePassword)
                    dismiss()
                }
                .keyboardShortcut(.defaultAction)
                .disabled(connection.name.isEmpty || connection.host.isEmpty)
            }
        }.padding(18).frame(width: 440, height: 440)
    }
}

// MARK: - Network Tools Workspace

private struct NetworkToolsWorkspace: View {
    @ObservedObject var model: DesktopModel
    @StateObject private var diagnosticsRunner = DiagnosticsRunner()
    @State private var pingTarget = ""
    @State private var pingMode = "ping"
    @State private var pingCount = 4

    @State private var arpRecords: [ArpRecord] = []
    @State private var arpFilter = ""
    @State private var isLoadingArp = false

    @State private var routeRecords: [RouteRecord] = []
    @State private var routeFilter = ""
    @State private var isLoadingRoute = false

    var body: some View {
        VStack(spacing: 0) {
            tabBar
            Divider()
            switch model.selectedToolTab {
            case .tcpTest:
                tcpTestView
            case .ping:
                pingView
            case .arp:
                arpView
            case .route:
                routeView
            }
        }
        .onAppear {
            if model.tcpTestHost.isEmpty, let first = model.connections.first {
                model.tcpTestHost = first.host
            }
            if pingTarget.isEmpty, let first = model.connections.first {
                pingTarget = first.host
            }
        }
    }

    private var tabBar: some View {
        HStack(spacing: 12) {
            ForEach(ToolTab.allCases) { tab in
                Button {
                    model.selectedToolTab = tab
                } label: {
                    HStack(spacing: 6) {
                        Image(systemName: tab.icon)
                        Text(tab.rawValue)
                    }
                    .font(.system(size: 12, weight: model.selectedToolTab == tab ? .semibold : .regular))
                    .padding(.horizontal, 12).padding(.vertical, 7)
                    .background(model.selectedToolTab == tab ? Color(nsColor: .selectedControlColor).opacity(0.18) : .clear, in: RoundedRectangle(cornerRadius: 6))
                }
                .buttonStyle(.plain)
            }
            Spacer()
        }
        .padding(.horizontal, 16).padding(.vertical, 8)
        .background(Color(nsColor: .controlBackgroundColor))
    }

    // MARK: 1. TCP Connection Test View

    private var tcpTestView: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                Text("TCP ポート接続テスト")
                    .font(.system(size: 15, weight: .semibold))

                Text("指定したホストおよびポートへの TCP ハンドシェイクを行い、到達可能性とレイテンシ（RTT）を計測します。")
                    .font(.system(size: 12)).foregroundStyle(.secondary)

                HStack(spacing: 10) {
                    VStack(alignment: .leading, spacing: 4) {
                        Text("ターゲット ホスト / IP").font(.system(size: 11, weight: .medium))
                        HStack {
                            TextField("192.168.1.1 または router.local", text: $model.tcpTestHost)
                                .textFieldStyle(.roundedBorder)

                            if !model.connections.isEmpty {
                                Menu {
                                    ForEach(model.connections) { conn in
                                        Button("\(conn.name) (\(conn.host))") {
                                            model.tcpTestHost = conn.host
                                            model.tcpTestPort = conn.port
                                        }
                                    }
                                } label: {
                                    Image(systemName: "list.bullet")
                                }
                                .menuStyle(.borderlessButton)
                                .help("登録機器から選択")
                            }
                        }
                    }
                    .frame(maxWidth: .infinity)

                    VStack(alignment: .leading, spacing: 4) {
                        Text("ポート").font(.system(size: 11, weight: .medium))
                        TextField("22", text: $model.tcpTestPort)
                            .textFieldStyle(.roundedBorder)
                            .frame(width: 80)
                    }

                    VStack(alignment: .leading, spacing: 4) {
                        Text("タイムアウト (ms)").font(.system(size: 11, weight: .medium))
                        TextField("2000", text: $model.tcpTestTimeout)
                            .textFieldStyle(.roundedBorder)
                            .frame(width: 90)
                    }

                    VStack(alignment: .leading, spacing: 4) {
                        Text(" ").font(.system(size: 11))
                        Button(action: model.testTcpDirect) {
                            HStack(spacing: 4) {
                                if model.isTestingTcp {
                                    ProgressView().controlSize(.small)
                                } else {
                                    Image(systemName: "bolt.fill")
                                }
                                Text("テスト実行")
                            }
                        }
                        .buttonStyle(.borderedProminent)
                        .disabled(model.tcpTestHost.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || model.isTestingTcp)
                    }
                }

                // Quick Port Presets
                HStack(spacing: 6) {
                    Text("プリセット:").font(.system(size: 11)).foregroundStyle(.secondary)
                    ForEach([("SSH", "22"), ("HTTP", "80"), ("HTTPS", "443"), ("Telnet", "23"), ("SNMP", "161"), ("Web (8080)", "8080")], id: \.1) { name, port in
                        Button("\(name) (\(port))") {
                            model.tcpTestPort = port
                        }
                        .buttonStyle(.bordered).controlSize(.mini)
                    }
                }

                // Result card
                if let result = model.tcpTestResult {
                    VStack(alignment: .leading, spacing: 8) {
                        HStack(spacing: 8) {
                            Circle()
                                .fill((model.tcpTestSuccess ?? false) ? Color.green : Color.red)
                                .frame(width: 10, height: 10)
                            Text((model.tcpTestSuccess ?? false) ? "接続成功" : "接続失敗")
                                .font(.system(size: 13, weight: .semibold))
                                .foregroundStyle((model.tcpTestSuccess ?? false) ? Color.green : Color.red)
                            Spacer()
                        }
                        Text(result)
                            .font(.system(size: 12, design: .monospaced))
                            .textSelection(.enabled)
                    }
                    .padding(12)
                    .background(Color(nsColor: .controlBackgroundColor), in: RoundedRectangle(cornerRadius: 8))
                    .overlay(RoundedRectangle(cornerRadius: 8).stroke(Color(nsColor: .separatorColor), lineWidth: 0.8))
                }

                // Recent tests log
                if !model.recentTcpTests.isEmpty {
                    VStack(alignment: .leading, spacing: 6) {
                        Text("最近のテスト結果").font(.system(size: 12, weight: .semibold)).foregroundStyle(.secondary)
                        VStack(alignment: .leading, spacing: 4) {
                            ForEach(model.recentTcpTests, id: \.self) { entry in
                                Text(entry)
                                    .font(.system(size: 11, design: .monospaced))
                                    .foregroundStyle(.secondary)
                            }
                        }
                        .padding(10)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .background(Color(nsColor: .controlBackgroundColor).opacity(0.6), in: RoundedRectangle(cornerRadius: 6))
                    }
                }
            }
            .padding(20)
            .frame(maxWidth: 820, alignment: .leading)
            .frame(maxWidth: .infinity, alignment: .topLeading)
        }
    }

    // MARK: 2. Ping / Traceroute View

    private var pingView: some View {
        VStack(spacing: 0) {
            HStack(spacing: 12) {
                Picker("", selection: $pingMode) {
                    Text("Ping").tag("ping")
                    Text("Traceroute").tag("traceroute")
                }
                .pickerStyle(.segmented)
                .frame(width: 180)

                TextField("ホスト名または IP アドレス", text: $pingTarget)
                    .textFieldStyle(.roundedBorder)

                if !model.connections.isEmpty {
                    Menu {
                        ForEach(model.connections) { conn in
                            Button("\(conn.name) (\(conn.host))") {
                                pingTarget = conn.host
                            }
                        }
                    } label: {
                        Image(systemName: "list.bullet")
                    }
                    .menuStyle(.borderlessButton)
                    .help("登録機器から選択")
                }

                if pingMode == "ping" {
                    Picker("回数", selection: $pingCount) {
                        Text("1回").tag(1)
                        Text("4回").tag(4)
                        Text("10回").tag(10)
                    }
                    .frame(width: 100)
                }

                if diagnosticsRunner.isRunning {
                    Button("停止", action: diagnosticsRunner.stop)
                        .buttonStyle(.borderedProminent)
                        .tint(.red)
                } else {
                    Button("実行") {
                        let target = pingTarget.trimmingCharacters(in: .whitespacesAndNewlines)
                        guard !target.isEmpty else { return }
                        if pingMode == "ping" {
                            diagnosticsRunner.run(command: "/sbin/ping", arguments: ["-c", "\(pingCount)", target])
                        } else {
                            diagnosticsRunner.run(command: "/usr/sbin/traceroute", arguments: ["-w", "2", "-m", "15", target])
                        }
                    }
                    .buttonStyle(.borderedProminent)
                    .disabled(pingTarget.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                }

                Button("クリア", action: diagnosticsRunner.clear)
                    .buttonStyle(.bordered)
                    .disabled(diagnosticsRunner.output.isEmpty)
            }
            .padding(14)
            .background(Color(nsColor: .windowBackgroundColor))
            Divider()

            ScrollViewReader { _ in
                ScrollView {
                    Text(diagnosticsRunner.output.isEmpty ? "Ping または Traceroute の実行結果がここにリアルタイムで表示されます。" : diagnosticsRunner.output)
                        .font(.system(size: 12, design: .monospaced))
                        .foregroundStyle(diagnosticsRunner.output.isEmpty ? .secondary : .primary)
                        .textSelection(.enabled)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(14)
                }
                .background(Color(nsColor: .textBackgroundColor))
            }
        }
    }

    // MARK: 3. ARP Table View

    private var arpView: some View {
        VStack(spacing: 0) {
            HStack {
                TextField("IP、MAC、インターフェースで検索…", text: $arpFilter)
                    .textFieldStyle(.roundedBorder)
                    .frame(maxWidth: 320)

                Spacer()

                Button("ARP テーブルを更新") {
                    isLoadingArp = true
                    Task.detached(priority: .userInitiated) {
                        let records = NetworkInspector.fetchArpTable()
                        await MainActor.run {
                            self.arpRecords = records
                            self.isLoadingArp = false
                        }
                    }
                }
                .buttonStyle(.bordered)
            }
            .padding(14)
            Divider()

            let filtered = arpRecords.filter { record in
                if arpFilter.isEmpty { return true }
                let query = arpFilter.lowercased()
                return record.ip.lowercased().contains(query)
                    || record.mac.lowercased().contains(query)
                    || record.interface.lowercased().contains(query)
            }

            if arpRecords.isEmpty {
                VStack(spacing: 8) {
                    Text("ARP テーブル未読み込み").font(.system(size: 13, weight: .semibold))
                    Text("上の「ARP テーブルを更新」ボタンをクリックして、ローカルマシンの ARP キャッシュを取得してください。").font(.system(size: 11)).foregroundStyle(.secondary)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                Table(filtered) {
                    TableColumn("IP アドレス", value: \.ip).width(min: 120, ideal: 140)
                    TableColumn("MAC アドレス", value: \.mac).width(min: 140, ideal: 160)
                    TableColumn("インターフェース", value: \.interface).width(80)
                    TableColumn("種別") { rec in
                        Text(rec.isPermanent ? "Permanent" : rec.isIncomplete ? "Incomplete" : "Dynamic")
                            .font(.system(size: 11))
                            .foregroundStyle(rec.isPermanent ? .blue : rec.isIncomplete ? .red : .secondary)
                    }.width(90)
                    TableColumn("アクション") { rec in
                        HStack(spacing: 6) {
                            Button("機器に追加") {
                                model.editingConnection = SavedConnection(name: rec.ip, host: rec.ip)
                            }
                            .buttonStyle(.borderless)
                            .font(.system(size: 11))

                            Button {
                                model.tcpTestHost = rec.ip
                                model.selectedToolTab = .tcpTest
                            } label: {
                                Image(systemName: "bolt.fill")
                            }
                            .buttonStyle(.borderless)
                            .help("接続テスト")
                        }
                    }.width(120)
                }
            }
        }
        .onAppear {
            if arpRecords.isEmpty {
                arpRecords = NetworkInspector.fetchArpTable()
            }
        }
    }

    // MARK: 4. Routing Table View

    private var routeView: some View {
        VStack(spacing: 0) {
            HStack {
                TextField("宛先、ゲートウェイ、インターフェースで検索…", text: $routeFilter)
                    .textFieldStyle(.roundedBorder)
                    .frame(maxWidth: 320)

                Spacer()

                Button("ルーティングテーブルを更新") {
                    isLoadingRoute = true
                    Task.detached(priority: .userInitiated) {
                        let records = NetworkInspector.fetchRoutingTable()
                        await MainActor.run {
                            self.routeRecords = records
                            self.isLoadingRoute = false
                        }
                    }
                }
                .buttonStyle(.bordered)
            }
            .padding(14)
            Divider()

            let filtered = routeRecords.filter { record in
                if routeFilter.isEmpty { return true }
                let query = routeFilter.lowercased()
                return record.destination.lowercased().contains(query)
                    || record.gateway.lowercased().contains(query)
                    || record.interface.lowercased().contains(query)
                    || record.flags.lowercased().contains(query)
            }

            if routeRecords.isEmpty {
                VStack(spacing: 8) {
                    Text("ルーティングテーブル未読み込み").font(.system(size: 13, weight: .semibold))
                    Text("上の「ルーティングテーブルを更新」ボタンをクリックして、ローカルマシンの経路情報を取得してください。").font(.system(size: 11)).foregroundStyle(.secondary)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                Table(filtered) {
                    TableColumn("宛先ネットワーク (Destination)", value: \.destination)
                    TableColumn("ゲートウェイ (Gateway)", value: \.gateway)
                    TableColumn("フラグ (Flags)", value: \.flags).width(70)
                    TableColumn("インターフェース (Netif)", value: \.interface).width(90)
                }
            }
        }
        .onAppear {
            if routeRecords.isEmpty {
                routeRecords = NetworkInspector.fetchRoutingTable()
            }
        }
    }
}

// MARK: - Settings Workspace

private struct SettingsWorkspace: View {
    @ObservedObject var model: DesktopModel

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                Text("モデル").font(.system(size: 13, weight: .semibold)).padding(.bottom, 12)
                HStack(spacing: 8) {
                    TextField("GGUF モデルファイル", text: $model.modelPath).textFieldStyle(.roundedBorder)
                    Button("選択…") { model.selectModel() }
                    Button(model.isLoadingModel ? "読み込み中…" : "読み込む") { model.loadModel() }
                        .disabled(model.modelPath.isEmpty || model.isLoadingModel)
                }
                Text(model.modelStatus).font(.system(size: 11)).foregroundStyle(model.modelStatus.hasPrefix("エラー") ? .red : .secondary).padding(.top, 6)
                Divider().padding(.vertical, 14)
                Text("ナレッジ").font(.system(size: 13, weight: .semibold)).padding(.bottom, 12)
                FolderPickerRow(title: "資料フォルダ", path: $model.documentsDirectory)
                Divider().padding(.vertical, 12)
                FolderPickerRow(title: "検索インデックス", path: $model.knowledgeDirectory)
                Text("変更した保存先は次の質問から Rust のナレッジ検索に使われます。既存の Tauri 版設定とは別に保存されます。")
                    .font(.system(size: 11)).foregroundStyle(.secondary).padding(.top, 8)
            }
            .frame(maxWidth: 720, alignment: .leading).padding(22)
            .frame(maxWidth: .infinity, alignment: .topLeading)
        }
    }
}

private struct FolderPickerRow: View {
    let title: String
    @Binding var path: String

    var body: some View {
        VStack(alignment: .leading, spacing: 7) {
            Text(title).font(.system(size: 12, weight: .medium))
            HStack(spacing: 8) {
                TextField("フォルダのパス", text: $path).textFieldStyle(.roundedBorder)
                Button("選択…") {
                    let panel = NSOpenPanel()
                    panel.canChooseDirectories = true
                    panel.canChooseFiles = false
                    panel.allowsMultipleSelection = false
                    if panel.runModal() == .OK, let url = panel.url { path = url.path }
                }
            }
        }
    }
}
