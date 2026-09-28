import SwiftUI
import AppKit
import Darwin
import MikomaiFFI

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
}

private struct ChatSession: Identifiable, Codable {
    var id = UUID()
    var title: String
    var messages: [ChatMessage] = []
    var updatedAt = Date()
}

private struct SavedConnection: Identifiable, Codable {
    var id = UUID()
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

@MainActor
private final class DesktopModel: ObservableObject {
    @Published var workspace: Workspace = .chat
    @Published var sessions: [ChatSession] = [] { didSet { persistSessions() } }
    @Published var activeSessionID: UUID? { didSet { persistActiveSession() } }
    @Published var draft = ""
    @Published var isWorking = false
    @Published var connections: [SavedConnection] = [] { didSet { persistConnections() } }
    @Published var editingConnection: SavedConnection?
    @Published var documentsDirectory: String { didSet { defaults.set(documentsDirectory, forKey: "mikomai.desktop.mac.documentsDirectory") } }
    @Published var knowledgeDirectory: String { didSet { defaults.set(knowledgeDirectory, forKey: "mikomai.desktop.mac.knowledgeDirectory") } }

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
        guard !prompt.isEmpty, !isWorking, let id = activeSessionID,
              let index = sessions.firstIndex(where: { $0.id == id }) else { return }
        sessions[index].messages.append(ChatMessage(role: .user, text: prompt))
        if sessions[index].messages.count == 1 { sessions[index].title = String(prompt.prefix(36)) }
        sessions[index].updatedAt = Date()
        let documents = documentsDirectory
        let knowledge = knowledgeDirectory
        draft = ""
        isWorking = true
        Task.detached(priority: .userInitiated) {
            let answer = Self.askRust(prompt, documents: documents, knowledge: knowledge)
            await MainActor.run {
                guard let index = self.sessions.firstIndex(where: { $0.id == id }) else { self.isWorking = false; return }
                self.sessions[index].messages.append(ChatMessage(role: .assistant, text: answer))
                self.sessions[index].updatedAt = Date()
                self.isWorking = false
            }
        }
    }

    func saveConnection(_ connection: SavedConnection) {
        guard connection.validationError == nil else { return }
        if let index = connections.firstIndex(where: { $0.id == connection.id }) { connections[index] = connection }
        else { connections.append(connection) }
        editingConnection = nil
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

    private nonisolated static func askRust(_ prompt: String, documents: String, knowledge: String) -> String {
        let response = prompt.withCString { message in
            documents.withCString { documentsPath in
                knowledge.withCString { knowledgePath in mikomai_chat_with_paths(message, documentsPath, knowledgePath) }
            }
        }
        defer { mikomai_result_free(response) }
        guard let message = response.message else { return "Rust 側から応答がありませんでした。" }
        let text = String(cString: message)
        return response.status == 0 ? text : "エラー: \(text)"
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
                            HStack(spacing: 8) { ProgressView().controlSize(.small); Text("資料を検索しています…").font(.system(size: 12)).foregroundStyle(.secondary) }
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
        HStack(alignment: .bottom, spacing: 10) {
            TextField("質問を入力…", text: $model.draft, axis: .vertical)
                .textFieldStyle(.plain).lineLimit(1...5).onSubmit(model.send).disabled(model.isWorking)
            Button(action: model.send) { Image(systemName: "arrow.up").font(.system(size: 12, weight: .semibold)).frame(width: 28, height: 28) }
                .buttonStyle(.borderedProminent).controlSize(.small).keyboardShortcut(.return, modifiers: [.command])
                .disabled(model.draft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || model.isWorking).help("送信")
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
                    Text(message.text).font(.system(size: 13)).textSelection(.enabled)
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
                Text("機器情報は Swift 版専用で保存されます。認証情報と実機への接続は未対応です。")
                    .font(.system(size: 11)).foregroundStyle(.secondary)
                Spacer()
                Button("CSV を読み込む") { importCSV() }
                Button("CSV を書き出す") { exportCSV() }.disabled(model.connections.isEmpty)
            }.padding(12).background(Color(nsColor: .controlBackgroundColor).opacity(0.5))
        }
        .alert("CSV", isPresented: $showsCSVAlert) { Button("OK", role: .cancel) {} } message: { Text(csvAlert) }
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
