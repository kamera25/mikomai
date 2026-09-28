import SwiftUI
import AppKit
import Darwin
import MikomaiFFI
import UniformTypeIdentifiers

@main
struct MikomaiDesktopMac: App {
    var body: some Scene {
        WindowGroup {
            DesktopWindow()
                .frame(minWidth: 920, minHeight: 620)
        }
        .windowStyle(.hiddenTitleBar)
    }
}

private enum Workspace: String, CaseIterable, Identifiable {
    case chat = "チャット"
    case connections = "機器情報一覧"
    case settings = "設定"

    var id: String { rawValue }
    var icon: String {
        switch self {
        case .chat: "bubble.left.and.bubble.right"
        case .connections: "point.3.connected.trianglepath.dotted"
        case .settings: "gearshape"
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

@MainActor
private final class DesktopModel: ObservableObject {
    @Published var workspace: Workspace = .chat
    @Published var sessions: [ChatSession] = [] { didSet { persistSessions() } }
    @Published var activeSessionID: UUID? { didSet { persistActiveSession() } }
    @Published var draft = ""
    @Published var pendingAttachments: [PendingAttachment] = []
    @Published var attachmentError = ""
    @Published var isWorking = false
    @Published var connections: [SavedConnection] = [] { didSet { persistConnections() } }
    @Published var editingConnection: SavedConnection?
    @Published var documentsDirectory: String { didSet { defaults.set(documentsDirectory, forKey: "mikomai.desktop.mac.documentsDirectory") } }
    @Published var knowledgeDirectory: String { didSet { defaults.set(knowledgeDirectory, forKey: "mikomai.desktop.mac.knowledgeDirectory") } }
    @Published var modelPath: String { didSet { defaults.set(modelPath, forKey: "mikomai.desktop.mac.modelPath") } }
    @Published var modelStatus = "モデル未ロード"
    @Published var isLoadingModel = false
    @Published var isCancelling = false

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
        sessions[index].updatedAt = Date()
        let documents = documentsDirectory
        let knowledge = knowledgeDirectory
        pendingAttachments = []
        attachmentError = ""
        draft = ""
        isWorking = true
        Task.detached(priority: .userInitiated) {
            let answer = Self.askRust(userText, history: history, documents: documents, knowledge: knowledge, attachments: attachmentText)
            await MainActor.run {
                guard let index = self.sessions.firstIndex(where: { $0.id == id }) else { self.isWorking = false; return }
                self.sessions[index].messages.append(ChatMessage(role: .assistant, text: answer))
                self.sessions[index].updatedAt = Date()
                self.isWorking = false
                self.isCancelling = false
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

    func saveConnection(_ connection: SavedConnection) {
        guard connection.validationError == nil else { return }
        if let index = connections.firstIndex(where: { $0.id == connection.id }) { connections[index] = connection }
        else { connections.append(connection) }
        editingConnection = nil
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

    private nonisolated static func askRust(_ prompt: String, history: String, documents: String, knowledge: String, attachments: String) -> String {
        let response = prompt.withCString { message in
            history.withCString { historyText in
                documents.withCString { documentsPath in
                    knowledge.withCString { knowledgePath in
                        attachments.withCString { attachmentText in
                            mikomai_assistant_chat_with_attachments(message, historyText, documentsPath, knowledgePath, attachmentText)
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

private struct DesktopWindow: View {
    @StateObject private var model = DesktopModel()

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
                    case .settings: SettingsWorkspace(model: model)
                    }
                }
            }
            .background(Color(nsColor: .windowBackgroundColor))
        }
        .background(Color(nsColor: .underPageBackgroundColor))
        .sheet(item: $model.editingConnection) { connection in
            ConnectionEditor(connection: connection) { model.saveConnection($0) }
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
                            HStack(spacing: 8) { ProgressView().controlSize(.small); Text(model.isCancelling ? "生成を停止しています…" : "資料を検索して回答を生成しています…").font(.system(size: 12)).foregroundStyle(.secondary) }
                                .padding(.leading, 42)
                        }
                    }
                    .frame(maxWidth: 760).frame(maxWidth: .infinity).padding(.horizontal, 24).padding(.vertical, 24)
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
            Text("機器の設定やトラブルシュートについて質問してください。").font(.system(size: 13)).foregroundStyle(.secondary)
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
                        if !language.isEmpty { Text(language).font(.system(size: 10, weight: .medium)).foregroundStyle(.secondary) }
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

private struct ConnectionsWorkspace: View {
    @ObservedObject var model: DesktopModel
    @State private var csvAlert = ""
    @State private var showsCSVAlert = false
    var body: some View {
        VStack(spacing: 0) {
            HStack {
                Text("機器情報").font(.system(size: 13, weight: .semibold))
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
                    Text("ネットワーク機器の接続情報を登録できます。").font(.system(size: 12)).foregroundStyle(.secondary)
                }.frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                Table(model.connections) {
                TableColumn("名前", value: \.name)
                TableColumn("登録元") { connection in
                    Text(connection.sourceID == nil ? "Mac 内" : "Tauri")
                        .font(.system(size: 11)).foregroundStyle(.secondary)
                }.width(62)
                TableColumn("ホスト", value: \.host)
                    TableColumn("ポート", value: \.port)
                    TableColumn("ユーザー", value: \.username)
                    TableColumn("機器タイプ", value: \.deviceType)
                    TableColumn("") { connection in
                        HStack(spacing: 8) {
                            Button { model.editingConnection = connection } label: { Image(systemName: "pencil") }.help("編集")
                            Button(role: .destructive) { model.connections.removeAll { $0.id == connection.id } } label: { Image(systemName: "trash") }.help("削除")
                        }.buttonStyle(.borderless)
                    }.width(68)
                }
            }
            Spacer(minLength: 0)
            HStack {
                Text("Tauri 機器はメタデータのみ複製します。認証情報・接続・同期には対応していません。")
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

private struct ConnectionEditor: View {
    @Environment(\.dismiss) private var dismiss
    @State var connection: SavedConnection
    let onSave: (SavedConnection) -> Void
    private let deviceTypes = ["Cisco IOS", "Cisco NX-OS", "Juniper JunOS", "Arista EOS", "Other"]

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text(connection.name.isEmpty ? "機器を追加" : "機器情報を編集").font(.system(size: 16, weight: .semibold))
            Form {
                TextField("名前", text: $connection.name)
                TextField("ホスト名または IP", text: $connection.host)
                TextField("ポート", text: $connection.port)
                TextField("ユーザー名", text: $connection.username)
                Picker("機器タイプ", selection: $connection.deviceType) { ForEach(deviceTypes, id: \.self) { Text($0) } }
            }.formStyle(.grouped)
            HStack { Spacer(); Button("キャンセル") { dismiss() }; Button("保存") { onSave(connection); dismiss() }.keyboardShortcut(.defaultAction).disabled(connection.name.isEmpty || connection.host.isEmpty) }
        }.padding(18).frame(width: 420, height: 350)
    }
}

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
