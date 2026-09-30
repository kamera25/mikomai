import SwiftUI
import AppKit
import Darwin
import Security
import MikomaiFFI
import MikomaiDesktopCore
import UniformTypeIdentifiers

// SwiftPM launches an unbundled executable. Explicitly register it as a
// foreground app so its windows can receive keyboard and IME events.
private final class DesktopAppDelegate: NSObject, NSApplicationDelegate {
    func applicationWillFinishLaunching(_ notification: Notification) {
        NSApplication.shared.setActivationPolicy(.regular)
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApplication.shared.activate(ignoringOtherApps: true)
    }
}

@main
struct MikomaiDesktopMac: App {
    @NSApplicationDelegateAdaptor(DesktopAppDelegate.self) private var appDelegate
    @StateObject private var model = DesktopModel()

    var body: some Scene {
        WindowGroup {
            DesktopWindow(model: model)
                .frame(minWidth: 1020, minHeight: 680)
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

// MARK: - Tauri Settings Model

private typealias TauriSettings = DesktopSettings

// MARK: - Model Presets

private let PRESET_MODELS = ModelPresetCatalog.presets

// MARK: - Hugging Face Hub Helper

private enum HuggingFaceHub {
    static var cacheDirectory: URL {
        if let env = ProcessInfo.processInfo.environment["HF_HUB_CACHE"], !env.isEmpty {
            return URL(fileURLWithPath: env)
        }
        if let envHome = ProcessInfo.processInfo.environment["HF_HOME"], !envHome.isEmpty {
            return URL(fileURLWithPath: envHome).appendingPathComponent("hub")
        }
        return FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".cache/huggingface/hub")
    }

    static func modelURL(repo: String, filename: String) -> URL {
        cacheDirectory.appendingPathComponent(repo).appendingPathComponent(filename)
    }

    static func modelExists(repo: String, filename: String) -> Bool {
        let url = modelURL(repo: repo, filename: filename)
        return FileManager.default.fileExists(atPath: url.path)
    }
}

// MARK: - Serial Port Helper

private enum SerialPortDetector {
    static func listPorts() -> [String] {
        let fm = FileManager.default
        guard let files = try? fm.contentsOfDirectory(atPath: "/dev") else { return [] }
        return files
            .filter { $0.hasPrefix("cu.") || $0.hasPrefix("tty.") }
            .map { "/dev/\($0)" }
            .sorted()
    }
}

// MARK: - Settings Manager (Tauri Config Auto-loader & Sync)

private enum SettingsManager {
    static var tauriSettingsURL: URL {
        if let env = ProcessInfo.processInfo.environment["MIKOMAI_SETTINGS_PATH"], !env.isEmpty {
            return URL(fileURLWithPath: env)
        }
        let fm = FileManager.default
        let appSupport = fm.urls(for: .applicationSupportDirectory, in: .userDomainMask).first
        let candidates = [
            appSupport?.appendingPathComponent("com.mikomai.agent/settings.json"),
            appSupport?.appendingPathComponent("mikomai/settings.json"),
            fm.homeDirectoryForCurrentUser.appendingPathComponent(".config/mikomai/settings.json")
        ].compactMap { $0 }

        for url in candidates {
            if fm.fileExists(atPath: url.path) {
                return url
            }
        }
        return appSupport?.appendingPathComponent("com.mikomai.agent/settings.json")
            ?? fm.homeDirectoryForCurrentUser.appendingPathComponent(".config/mikomai/settings.json")
    }

    static func loadFromTauri() -> (settings: TauriSettings, url: URL, isLoaded: Bool) {
        let url = tauriSettingsURL
        guard FileManager.default.fileExists(atPath: url.path),
              let data = try? Data(contentsOf: url),
              let decoded = try? DesktopSettingsCodec.decode(data) else {
            return (TauriSettings(), url, false)
        }
        return (decoded, url, true)
    }

    static func saveToTauri(_ settings: TauriSettings) throws {
        let url = tauriSettingsURL
        let dir = url.deletingLastPathComponent()
        if !FileManager.default.fileExists(atPath: dir.path) {
            try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        }
        let data = try DesktopSettingsCodec.encode(settings)
        let tempURL = url.appendingPathExtension("tmp")
        try data.write(to: tempURL, options: .atomic)
        if FileManager.default.fileExists(atPath: url.path) {
            _ = try? FileManager.default.removeItem(at: url)
        }
        try FileManager.default.moveItem(at: tempURL, to: url)
    }
}

// MARK: - Chat & Saved Connections Models

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

private struct KeychainCredentialAdapter: CredentialStore {
    func save(key: String, value: String) { KeychainHelper.save(key: key, value: value) }
    func load(key: String) -> String? { KeychainHelper.load(key: key) }
    func delete(key: String) { KeychainHelper.delete(key: key) }
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

    // Knowledge dirs
    @Published var documentsDirectory: String { didSet { defaults.set(documentsDirectory, forKey: "mikomai.desktop.mac.documentsDirectory") } }
    @Published var knowledgeDirectory: String { didSet { defaults.set(knowledgeDirectory, forKey: "mikomai.desktop.mac.knowledgeDirectory") } }

    // Model path & status
    @Published var modelPath: String = "" { didSet { defaults.set(modelPath, forKey: "mikomai.desktop.mac.modelPath") } }
    @Published var modelStatus = "モデル未ロード"
    @Published var isLoadingModel = false
    @Published var isCancelling = false

    // Tauri Settings
    @Published var settings: TauriSettings = TauriSettings()
    @Published var tauriConfigURL: URL = SettingsManager.tauriSettingsURL
    @Published var isTauriConfigLoaded: Bool = false
    @Published var tauriSyncMessage: String = ""

    // Model Presets & HuggingFace
    @Published var selectedPresetId: String = "gemma-4-e4b-ud"
    @Published var repoPath: String = "unsloth/gemma-4-E4B-it-GGUF"
    @Published var modelFilename: String = "gemma-4-E4B-it-UD-Q4_K_XL.gguf"
    @Published var isDownloadingModel: Bool = false
    @Published var downloadProgressText: String = ""

    // Tools state
    @Published var tcpTestHost = ""
    @Published var tcpTestPort = "22"
    @Published var tcpTestTimeout = "2000"
    @Published var isTestingTcp = false
    @Published var tcpTestResult: String?
    @Published var tcpTestSuccess: Bool?
    @Published var recentTcpTests: [String] = []

    private let defaults = UserDefaults.standard
    private let credentialPersistence = ConnectionCredentialPersistence(store: KeychainCredentialAdapter())
    private let sessionsKey = "mikomai.desktop.mac.sessions.v1"
    private let activeKey = "mikomai.desktop.mac.activeSession.v1"
    private let connectionsKey = "mikomai.desktop.mac.connections.v1"

    init() {
        let bundledDocuments = Bundle.main.resourceURL?.appendingPathComponent("nw-docs", isDirectory: true).path
        let repoRootDocuments = URL(fileURLWithPath: FileManager.default.currentDirectoryPath).appendingPathComponent("nw-docs").path
        let defaultDocuments: String = {
            if let bundled = bundledDocuments, FileManager.default.fileExists(atPath: bundled) {
                return bundled
            }
            if FileManager.default.fileExists(atPath: repoRootDocuments) {
                return repoRootDocuments
            }
            return bundledDocuments ?? repoRootDocuments
        }()
        let defaultKnowledge = (FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first ?? FileManager.default.temporaryDirectory)
            .appendingPathComponent("MikomaiDesktopMac/knowledge", isDirectory: true).path
        let rawDocs = defaults.string(forKey: "mikomai.desktop.mac.documentsDirectory")
            ?? ProcessInfo.processInfo.environment["MIKOMAI_DOCS_DIR"] ?? defaultDocuments
        let rawKnowledge = defaults.string(forKey: "mikomai.desktop.mac.knowledgeDirectory")
            ?? ProcessInfo.processInfo.environment["MIKOMAI_KNOWLEDGE_DIR"] ?? defaultKnowledge
        documentsDirectory = (rawDocs as NSString).expandingTildeInPath
        knowledgeDirectory = (rawKnowledge as NSString).expandingTildeInPath

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

        // Automatically load Tauri settings
        loadTauriConfig()

        refreshModelStatus()
    }

    var activeSession: ChatSession? { sessions.first(where: { $0.id == activeSessionID }) }

    func createSession() {
        var state = ChatSessionState(sessions: sessions, activeSessionID: activeSessionID)
        _ = state.create()
        sessions = state.sessions
        activeSessionID = state.activeSessionID
        workspace = .chat
    }

    func select(_ id: UUID) {
        var state = ChatSessionState(sessions: sessions, activeSessionID: activeSessionID)
        state.select(id)
        guard state.activeSessionID == id else { return }
        activeSessionID = state.activeSessionID
        workspace = .chat
    }

    func deleteSession(_ id: UUID) {
        var state = ChatSessionState(sessions: sessions, activeSessionID: activeSessionID)
        state.delete(id)
        sessions = state.sessions
        activeSessionID = state.activeSessionID
    }

    func renameSession(_ id: UUID, title: String) {
        var state = ChatSessionState(sessions: sessions, activeSessionID: activeSessionID)
        state.rename(id, to: title)
        sessions = state.sessions
    }

    // MARK: - Tauri Settings Management

    func loadTauriConfig() {
        let (loadedSettings, url, isLoaded) = SettingsManager.loadFromTauri()
        self.settings = loadedSettings
        self.tauriConfigURL = url
        self.isTauriConfigLoaded = isLoaded

        if isLoaded {
            self.tauriSyncMessage = "Tauri 版設定を自動読み込みしました: \(url.path)"
            if let path = loadedSettings.modelPath, !path.isEmpty {
                let expanded = (path as NSString).expandingTildeInPath
                self.modelPath = expanded
                // Check if matching preset
                let fname = FilePathPolicy.defaultFilename(expanded)
                if let match = ModelPresetCatalog.find(filename: fname) {
                    self.selectedPresetId = match.id
                    self.repoPath = match.repo
                    self.modelFilename = match.filename
                } else {
                    self.selectedPresetId = "custom"
                    self.modelFilename = fname
                }
                if FileManager.default.fileExists(atPath: expanded) {
                    loadModel()
                }
            }
        } else {
            let savedPath = defaults.string(forKey: "mikomai.desktop.mac.modelPath") ?? ""
            self.modelPath = (savedPath as NSString).expandingTildeInPath
            self.tauriSyncMessage = "Tauri 版設定ファイルが見つかりません。デフォルト値を使用しています: \(url.path)"
            if !self.modelPath.isEmpty && FileManager.default.fileExists(atPath: self.modelPath) {
                loadModel()
            }
        }

        applyInferenceParams()
    }

    func saveTauriConfig() {
        var toSave = settings
        if !modelPath.isEmpty {
            var patch = DesktopSettingsPatch()
            patch.modelPath = .set(modelPath)
            toSave.merge(patch)
        }
        do {
            try SettingsManager.saveToTauri(toSave)
            self.isTauriConfigLoaded = true
            self.tauriSyncMessage = "Tauri 版設定ファイルに保存しました: \(tauriConfigURL.path)"
            applyInferenceParams()
        } catch {
            self.tauriSyncMessage = "設定の保存に失敗しました: \(error.localizedDescription)"
        }
    }

    func resetSettingsToDefault() {
        self.settings = TauriSettings()
        saveTauriConfig()
        applyInferenceParams()
        self.tauriSyncMessage = "設定をデフォルト値にリセットしました。"
    }

    func applyInferenceParams() {
        let temp = Float(settings.temperature)
        let rep = Float(settings.repetitionPenalty)
        let nCtx = UInt32(settings.nCtx)
        let maxGen = UInt32(settings.maxGen)
        _ = Self.callRust {
            mikomai_set_inference_params(temp, rep, nCtx, maxGen)
        }
    }

    func selectPreset(_ presetId: String) {
        selectedPresetId = presetId
        if presetId != "custom", let preset = PRESET_MODELS.first(where: { $0.id == presetId }) {
            repoPath = preset.repo
            modelFilename = preset.filename
            let cachedURL = HuggingFaceHub.modelURL(repo: preset.repo, filename: preset.filename)
            if FileManager.default.fileExists(atPath: cachedURL.path) {
                modelPath = cachedURL.path
            }
        }
    }

    // MARK: - Streaming Chat

    func send() {
        let prompt = ChatSubmissionPolicy.normalizedPrompt(draft)
        guard ChatSubmissionPolicy.shouldSubmit(
            prompt: prompt,
            attachmentCount: pendingAttachments.count,
            isWorking: isWorking
        ) else { return }
        let recentHostCandidates = Self.recentHostCandidates(in: prompt)
        if !recentHostCandidates.isEmpty {
            let updated = HostSuggestionPolicy.updateRecentHosts(recentHostCandidates, current: settings.recentIps)
            if updated != settings.recentIps {
                settings.recentIps = updated
                saveTauriConfig()
            }
        }
        if activeSessionID == nil || !sessions.contains(where: { $0.id == activeSessionID }) {
            createSession()
        }
        guard let id = activeSessionID, let index = sessions.firstIndex(where: { $0.id == id }) else { return }

        // Context limit derived from settings.historyLimit
        let maxHistoryTurns = max(2, settings.historyLimit * 2)
        let history = sessions[index].messages.suffix(maxHistoryTurns).map { message in
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

        let documents = (documentsDirectory as NSString).expandingTildeInPath
        let knowledge = (knowledgeDirectory as NSString).expandingTildeInPath
        let modelP = (modelPath as NSString).expandingTildeInPath
        pendingAttachments = []
        attachmentError = ""
        draft = ""
        isWorking = true

        Task.detached(priority: .userInitiated) {
            // Auto-load model if configured but not yet loaded in Rust FFI
            let currentLoaded = Self.callRust { mikomai_model_status() }
            if currentLoaded.isEmpty && !modelP.isEmpty && FileManager.default.fileExists(atPath: modelP) {
                _ = Self.callRust { modelP.withCString { mikomai_model_load($0) } }
                await MainActor.run {
                    self.refreshModelStatus()
                }
            }

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
                if finalAnswer.hasPrefix("エラー:") {
                    if self.sessions[sIdx].messages[mIdx].text.isEmpty {
                        self.sessions[sIdx].messages[mIdx].text = finalAnswer
                    } else {
                        self.sessions[sIdx].messages[mIdx].text += "\n\n[\(finalAnswer)]"
                    }
                } else if self.sessions[sIdx].messages[mIdx].text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                    self.sessions[sIdx].messages[mIdx].text = finalAnswer
                }
                self.sessions[sIdx].updatedAt = Date()
                self.isWorking = false
                self.isCancelling = false
                self.persistSessions()
            }
        }
    }

    private static func recentHostCandidates(in text: String) -> [String] {
        let pattern = #"@([a-zA-Z0-9.-]+)|\b(?:\d{1,3}\.){3}\d{1,3}\b"#
        guard let regex = try? NSRegularExpression(pattern: pattern) else { return [] }
        let range = NSRange(text.startIndex..., in: text)
        var seen = Set<String>()
        return regex.matches(in: text, range: range).compactMap { match in
            let capture = match.range(at: 1).location == NSNotFound ? match.range : match.range(at: 1)
            guard let swiftRange = Range(capture, in: text) else { return nil }
            let value = String(text[swiftRange])
            return seen.insert(value).inserted ? value : nil
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

        var loaded = pendingAttachments
        var totalBytes = loaded.reduce(0) { $0 + $1.byteCount }
        for url in panel.urls {
            guard !loaded.contains(where: { $0.name == url.lastPathComponent }) else { continue }
            let hasScope = url.startAccessingSecurityScopedResource()
            defer { if hasScope { url.stopAccessingSecurityScopedResource() } }
            do {
                let handle = try FileHandle(forReadingFrom: url)
                defer { try? handle.close() }
                let data = try handle.read(upToCount: TextAttachmentPolicy.maxFileBytes + 1) ?? Data()
                let attachment = try TextAttachmentPolicy.prepare(
                    name: url.lastPathComponent,
                    data: data,
                    existingNames: Set(loaded.map(\.name)),
                    currentTotalBytes: totalBytes
                )
                loaded.append(attachment)
                totalBytes += attachment.byteCount
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
        if panel.runModal() == .OK, let url = panel.url {
            modelPath = url.path
            settings.modelPath = url.path
            saveTauriConfig()
        }
    }

    func loadModel() {
        let path = (modelPath as NSString).expandingTildeInPath
        guard !path.isEmpty, !isLoadingModel else { return }
        isLoadingModel = true
        modelStatus = "モデルを読み込み中…"
        applyInferenceParams()
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

    func openModelDirectory() {
        let dir = HuggingFaceHub.cacheDirectory
        NSWorkspace.shared.open(dir)
    }

    func openTauriConfigDirectory() {
        let dir = tauriConfigURL.deletingLastPathComponent()
        NSWorkspace.shared.open(dir)
    }

    // MARK: - Connections & Keychain

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

    func importTauriDevices(fromJSON data: Data) throws -> TauriConnectionImportResult {
        let result = try TauriConnectionImporter.importJSON(data, existing: connections)
        connections.append(contentsOf: result.imported)
        return result
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
    @State private var suggestionVisibility = ChatSuggestionVisibilityState()
    @FocusState private var isChatInputFocused: Bool

    private var hostSuggestionContext: (query: String, atIndex: String.Index)? {
        guard let atIndex = model.draft.lastIndex(of: "@") else { return nil }
        let queryStart = model.draft.index(after: atIndex)
        let query = String(model.draft[queryStart...])
        guard !query.contains(where: \.isWhitespace) else { return nil }
        return (query, atIndex)
    }

    private var hostSuggestions: [HostSuggestion] {
        guard let context = hostSuggestionContext else { return [] }
        let hosts = model.connections.map { HostSuggestion(hostname: $0.name, ip: $0.host) }
        return HostSuggestionPolicy.find(
            query: context.query,
            availableHosts: hosts,
            recentIPs: model.settings.recentIps,
            labels: HostSuggestionLabels(localhost: "このコンピュータ", pastIps: "過去に投入したIPアドレス")
        )
    }

    private func selectHostSuggestion(_ suggestion: HostSuggestion) {
        guard let context = hostSuggestionContext else { return }
        model.draft.replaceSubrange(context.atIndex..., with: "\(suggestion.hostname) ")
    }

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
            if model.isTauriConfigLoaded {
                HStack(spacing: 4) {
                    Image(systemName: "arrow.triangle.2.circlepath")
                        .font(.system(size: 9))
                    Text("Tauri 設定同期中")
                        .font(.system(size: 10))
                }
                .padding(.horizontal, 8).padding(.vertical, 3)
                .background(Color.green.opacity(0.12), in: Capsule())
                .foregroundStyle(.green)
            }
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
            if suggestionVisibility.isVisible && hostSuggestionContext != nil && !hostSuggestions.isEmpty {
                VStack(alignment: .leading, spacing: 2) {
                    ForEach(hostSuggestions) { suggestion in
                        let icon: String = {
                            if suggestion.hostname == "localhost" { return "desktopcomputer" }
                            if suggestion.ip == "過去に投入したIPアドレス",
                               IPAddressPolicy.isGlobalIP(suggestion.hostname) {
                                return "globe"
                            }
                            return "point.3.connected.trianglepath.dotted"
                        }()
                        Button {
                            selectHostSuggestion(suggestion)
                        } label: {
                            HStack(spacing: 8) {
                                Image(systemName: icon).font(.system(size: 11)).foregroundStyle(.secondary)
                                Text(suggestion.hostname).font(.system(size: 12, weight: .medium))
                                Text(suggestion.ip).font(.system(size: 11)).foregroundStyle(.secondary)
                                Spacer(minLength: 0)
                            }
                            .contentShape(Rectangle())
                            .padding(.horizontal, 8)
                            .padding(.vertical, 5)
                        }
                        .buttonStyle(.plain)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(4)
                .background(Color(nsColor: .controlBackgroundColor), in: RoundedRectangle(cornerRadius: 5))
            }
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

                TextField("質問を入力… (⌘+Enter で送信)", text: $model.draft, axis: .vertical)
                    .textFieldStyle(.plain)
                    .lineLimit(1...6)
                    .focused($isChatInputFocused)
                    .font(.system(size: 13))
                    .disabled(model.isWorking)
                    .onExitCommand { suggestionVisibility.dismissForEscape() }
                    .padding(.horizontal, 4)
                    .padding(.vertical, 4)

                if model.isWorking {
                    Button(action: model.stop) { Image(systemName: "stop.fill").font(.system(size: 10, weight: .semibold)).frame(width: 28, height: 28) }
                        .buttonStyle(.bordered).controlSize(.small)
                        .disabled(!ChatSubmissionPolicy.canStop(isWorking: model.isWorking, isCancelling: model.isCancelling))
                        .help("生成を停止")
                } else {
                    Button(action: model.send) { Image(systemName: "arrow.up").font(.system(size: 12, weight: .semibold)).frame(width: 28, height: 28) }
                        .buttonStyle(.borderedProminent).controlSize(.small).keyboardShortcut(.return, modifiers: [.command])
                        .disabled(!ChatSubmissionPolicy.hasContent(prompt: model.draft, attachmentCount: model.pendingAttachments.count))
                        .help("送信 (⌘+Enter)")
                }
            }
        }
        .padding(10).background(Color(nsColor: .textBackgroundColor)).clipShape(RoundedRectangle(cornerRadius: 7))
        .overlay(RoundedRectangle(cornerRadius: 7).stroke(Color(nsColor: .separatorColor), lineWidth: 0.7))
        .frame(maxWidth: 760).padding(.horizontal, 22).padding(.top, 10).padding(.bottom, 14)
        .frame(maxWidth: .infinity).background(Color(nsColor: .windowBackgroundColor))
        .onAppear {
            refreshSuggestionVisibilityForInput()
            isChatInputFocused = !model.isWorking
        }
        .onChange(of: model.isWorking) { isWorking in
            if !isWorking { isChatInputFocused = true }
        }
        .onChange(of: model.draft) { _ in refreshSuggestionVisibilityForInput() }
        .onChange(of: hostSuggestions.map(\.hostname)) { _ in
            suggestionVisibility.updateCandidates(count: hostSuggestions.count)
        }
    }

    private func refreshSuggestionVisibilityForInput() {
        suggestionVisibility.updateForInput(
            hasMentionQuery: hostSuggestionContext != nil,
            candidateCount: hostSuggestions.count
        )
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
                        if connection.hasPassword || connection.hasEnablePassword {
                            Label(
                                connection.hasPassword && connection.hasEnablePassword ? "Key + Enable" :
                                    (connection.hasEnablePassword ? "Enable" : "Key"),
                                systemImage: "key.fill"
                            ).font(.system(size: 11)).foregroundStyle(.green)
                        } else {
                            Text("未設定").font(.system(size: 11)).foregroundStyle(.secondary)
                        }
                    }.width(90)
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
        let result: TauriConnectionImportResult
        do {
            result = try model.importTauriDevices(fromJSON: Data(json.utf8))
        } catch {
            presentCSVMessage("Tauri の機器情報 JSON を読み取れませんでした。元ファイルは変更していません。")
            return
        }
        var note = "\(result.imported.count) 件を追加し、\(result.skipped) 件をスキップしました。元ファイルは変更していません。"
        if result.missingIDs > 0 { note += " IDのない行は重複判定せず追加しました。" }
        presentCSVMessage(note)
    }

    private func exportCSV() {
        let panel = NSSavePanel(); panel.nameFieldStringValue = "connections.csv"
        guard panel.runModal() == .OK, let url = panel.url else { return }
        if let invalid = model.connections.first(where: { $0.validationError != nil }) {
            presentCSVMessage("\(invalid.name) のホスト名が Tauri CSV 形式の制約に合いません。機器情報を編集してください。")
            return
        }
        do {
            let csv = try ConnectionCSVCodec.exportCSV(model.connections)
            try csv.write(to: url, atomically: true, encoding: .utf8)
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
        do {
            let result = try ConnectionCSVCodec.importCSV(content, existing: model.connections)
            model.connections = result.connections
            let details = result.warnings.prefix(5).map { "\($0.row)行目: \($0.reason)" }
            var message = "\(result.importedCount) 件を読み込みました。\(result.warnings.count) 件は形式が合わないためスキップしました。"
            if !details.isEmpty { message += "\n" + details.joined(separator: "\n") }
            if result.warnings.count > details.count { message += "\nほか \(result.warnings.count - details.count) 件" }
            presentCSVMessage(message)
        } catch {
            presentCSVMessage(error.localizedDescription)
        }
    }

    private func presentCSVMessage(_ message: String) {
        csvAlert = message
        showsCSVAlert = true
    }

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
        let credentials = ConnectionCredentialPersistence(store: KeychainCredentialAdapter()).load(for: connection.id)
        self._password = State(initialValue: credentials.password ?? "")
        self._enablePassword = State(initialValue: credentials.enablePassword ?? "")
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text(connection.name.isEmpty ? "機器を追加" : "機器情報を編集").font(.system(size: 16, weight: .semibold))
            Form {
                TextField("名前", text: $connection.name)
                TextField("ホスト名または IP", text: $connection.host)
                TextField("ポート", text: $connection.port)
                TextField("ユーザー名", text: $connection.username)
                Picker("接続方式", selection: Binding(
                    get: { connection.connectionType ?? "SSH" },
                    set: { connection.connectionType = $0 }
                )) {
                    Text("SSH").tag("SSH")
                    Text("Console").tag("Console")
                }
                Picker("機器タイプ", selection: $connection.deviceType) { ForEach(deviceTypes, id: \.self) { Text($0) } }

                Section("資格情報 (Keychain)") {
                    SecureField("パスワード", text: $password)
                    SecureField("Enable パスワード", text: $enablePassword)
                    Text("パスワードは macOS Keychain に暗号化されて安全に保管されます。平文ファイルには保存されません。")
                        .font(.system(size: 10)).foregroundStyle(.secondary)
                }
            }
            .formStyle(.grouped)
            if let validationError = connection.validationError {
                Text(validationError).font(.system(size: 11)).foregroundStyle(.red)
            }
            HStack {
                Spacer()
                Button("キャンセル") { dismiss() }
                Button("保存") {
                    guard connection.validationError == nil else { return }
                    onSave(connection, password, enablePassword)
                    dismiss()
                }
                .keyboardShortcut(.defaultAction)
                .disabled(connection.validationError != nil)
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

// MARK: - Full Settings Workspace (Complete Port of Tauri AppSettings)

private struct SettingsWorkspace: View {
    @ObservedObject var model: DesktopModel
    @State private var selectedCategory = 0
    @State private var availablePorts: [String] = []

    var body: some View {
        VStack(spacing: 0) {
            // Category Segmented Control
            Picker("", selection: $selectedCategory) {
                Text("チャット・通信").tag(0)
                Text("LLM モデル").tag(1)
                Text("Vision (画像)").tag(2)
                Text("ナレッジ RAG").tag(3)
                Text("Tauri 同期").tag(4)
            }
            .pickerStyle(.segmented)
            .padding(.horizontal, 20).padding(.vertical, 10)
            .background(Color(nsColor: .controlBackgroundColor))

            Divider()

            ScrollView {
                VStack(alignment: .leading, spacing: 20) {
                    switch selectedCategory {
                    case 0:
                        chatAndNetworkSection
                    case 1:
                        llmModelSection
                    case 2:
                        visionSection
                    case 3:
                        knowledgeSection
                    case 4:
                        tauriSyncSection
                    default:
                        EmptyView()
                    }
                }
                .padding(24)
                .frame(maxWidth: 820, alignment: .leading)
                .frame(maxWidth: .infinity, alignment: .topLeading)
            }
        }
        .onAppear {
            availablePorts = SerialPortDetector.listPorts()
        }
    }

    // MARK: Category 0: Chat & Network Settings

    private var chatAndNetworkSection: some View {
        VStack(alignment: .leading, spacing: 18) {
            Text("チャット・通信設定").font(.system(size: 15, weight: .semibold))

            // History Limit
            VStack(alignment: .leading, spacing: 6) {
                HStack {
                    Text("会話履歴の上限 (ターン数)")
                    Spacer()
                    Text("\(model.settings.historyLimit)").font(.system(size: 12, design: .monospaced)).bold()
                }
                Slider(value: Binding(
                    get: { Double(model.settings.historyLimit) },
                    set: { model.settings.historyLimit = Int($0); model.saveTauriConfig() }
                ), in: 0...20, step: 1)
                Text("モデルに送信する直近の会話履歴の最大往復数です (0〜20)。").font(.system(size: 11)).foregroundStyle(.secondary)
            }

            Divider()

            // Temperature
            VStack(alignment: .leading, spacing: 6) {
                HStack {
                    Text("サンプリング温度 (Temperature)")
                    Spacer()
                    Text(String(format: "%.1f", model.settings.temperature)).font(.system(size: 12, design: .monospaced)).bold()
                }
                Slider(value: Binding(
                    get: { model.settings.temperature },
                    set: { model.settings.temperature = $0; model.saveTauriConfig() }
                ), in: 0.0...2.0, step: 0.1)
                Text("生成される回答のランダム性を調整します。ネットワーク設定には 0.0〜0.2 の決定的な値が推奨されます。").font(.system(size: 11)).foregroundStyle(.secondary)
            }

            Divider()

            // Repetition Penalty
            VStack(alignment: .leading, spacing: 6) {
                HStack {
                    Text("繰り返しペナルティ (Repetition Penalty)")
                    Spacer()
                    Text(String(format: "%.2f", model.settings.repetitionPenalty)).font(.system(size: 12, design: .monospaced)).bold()
                }
                Slider(value: Binding(
                    get: { model.settings.repetitionPenalty },
                    set: { model.settings.repetitionPenalty = $0; model.saveTauriConfig() }
                ), in: 1.0...2.0, step: 0.05)
                Text("同じ単語や句の重複を抑えるペナルティ係数です (1.0〜2.0、デフォルト: 1.10)。").font(.system(size: 11)).foregroundStyle(.secondary)
            }

            Divider()

            // MCP Timeout
            VStack(alignment: .leading, spacing: 6) {
                HStack {
                    Text("MCP / ツール実行タイムアウト (秒)")
                    Spacer()
                    Text("\(model.settings.mcpTimeout ?? 30) 秒").font(.system(size: 12, design: .monospaced)).bold()
                }
                Slider(value: Binding(
                    get: { Double(model.settings.mcpTimeout ?? 30) },
                    set: { model.settings.mcpTimeout = Int($0); model.saveTauriConfig() }
                ), in: 5...120, step: 5)
                Text("ネットワークツールやコマンド実行の待機タイムアウト時間です。").font(.system(size: 11)).foregroundStyle(.secondary)
            }

            Divider()

            // Cache Expiry
            VStack(alignment: .leading, spacing: 6) {
                HStack {
                    Text("事実グラフ・キャッシュ有効期限 (分)")
                    Spacer()
                    Text("\(model.settings.cacheExpiryMinutes ?? 10) 分").font(.system(size: 12, design: .monospaced)).bold()
                }
                Slider(value: Binding(
                    get: { Double(model.settings.cacheExpiryMinutes ?? 10) },
                    set: { model.settings.cacheExpiryMinutes = Int($0); model.saveTauriConfig() }
                ), in: 0...60, step: 1)
                Text("ネットワークトポロジ事実キャッシュの保持時間です (0 = キャッシュ無効)。").font(.system(size: 11)).foregroundStyle(.secondary)
            }

            Divider()

            // IP Version
            HStack {
                VStack(alignment: .leading, spacing: 2) {
                    Text("優先 IP バージョン").font(.system(size: 13, weight: .medium))
                    Text("Ping や接続テスト時に優先する IP プロトコルを指定します。").font(.system(size: 11)).foregroundStyle(.secondary)
                }
                Spacer()
                Picker("", selection: Binding(
                    get: { model.settings.ipVersion ?? "auto" },
                    set: { model.settings.ipVersion = $0; model.saveTauriConfig() }
                )) {
                    Text("自動判定 (Auto)").tag("auto")
                    Text("IPv4").tag("ipv4")
                    Text("IPv6").tag("ipv6")
                }
                .frame(width: 140)
            }

            Divider()

            // Auto Dry-Run
            Toggle(isOn: Binding(
                get: { model.settings.autoDryRun },
                set: { model.settings.autoDryRun = $0; model.saveTauriConfig() }
            )) {
                VStack(alignment: .leading, spacing: 2) {
                    Text("自動 Dry-Run 検証").font(.system(size: 13, weight: .medium))
                    Text("設定投入前に自動的にドライラン構文チェックを行います。").font(.system(size: 11)).foregroundStyle(.secondary)
                }
            }

            Divider()

            // Console Port & Baud Rate
            VStack(alignment: .leading, spacing: 10) {
                Text("シリアルコンソール設定").font(.system(size: 13, weight: .medium))
                HStack(spacing: 12) {
                    Picker("ポート", selection: Binding(
                        get: { model.settings.consolePort ?? "" },
                        set: { model.settings.consolePort = $0.isEmpty ? nil : $0; model.saveTauriConfig() }
                    )) {
                        Text("未設定 (None)").tag("")
                        ForEach(availablePorts, id: \.self) { port in
                            Text(port).tag(port)
                        }
                    }
                    .frame(maxWidth: .infinity)

                    Picker("ボーレート", selection: Binding(
                        get: { model.settings.consoleBaudRate ?? 9600 },
                        set: { model.settings.consoleBaudRate = $0; model.saveTauriConfig() }
                    )) {
                        Text("9600 bps").tag(9600)
                        Text("19200 bps").tag(19200)
                        Text("38400 bps").tag(38400)
                        Text("57600 bps").tag(57600)
                        Text("115200 bps").tag(115200)
                    }
                    .frame(width: 140)
                }
            }
        }
    }

    // MARK: Category 1: LLM Model Settings

    private var llmModelSection: some View {
        VStack(alignment: .leading, spacing: 18) {
            Text("ローカル LLM モデル設定").font(.system(size: 15, weight: .semibold))

            // Presets
            VStack(alignment: .leading, spacing: 6) {
                Text("モデルプリセット").font(.system(size: 12, weight: .medium))
                Picker("", selection: Binding(
                    get: { model.selectedPresetId },
                    set: { model.selectPreset($0) }
                )) {
                    ForEach(PRESET_MODELS) { preset in
                        let exists = HuggingFaceHub.modelExists(repo: preset.repo, filename: preset.filename)
                        Text("\(preset.name) \(exists ? "(✓ DL済)" : "(未DL)")").tag(preset.id)
                    }
                    Text("カスタムモデル (任意の GGUF)").tag("custom")
                }
                .pickerStyle(.radioGroup)
            }

            // Hugging Face Repo & Filename
            VStack(alignment: .leading, spacing: 10) {
                HStack(spacing: 8) {
                    VStack(alignment: .leading, spacing: 2) {
                        Text("Hugging Face リポジトリ").font(.system(size: 11, weight: .medium))
                        TextField("unsloth/gemma-4-E4B-it-GGUF", text: $model.repoPath)
                            .textFieldStyle(.roundedBorder)
                            .disabled(model.selectedPresetId != "custom")
                    }

                    VStack(alignment: .leading, spacing: 2) {
                        Text("GGUF ファイル名").font(.system(size: 11, weight: .medium))
                        TextField("gemma-4-E4B-it-UD-Q4_K_XL.gguf", text: $model.modelFilename)
                            .textFieldStyle(.roundedBorder)
                            .disabled(model.selectedPresetId != "custom")
                    }
                }

                // Presence check
                let exists = HuggingFaceHub.modelExists(repo: model.repoPath, filename: model.modelFilename)
                HStack(spacing: 6) {
                    Circle().fill(exists ? Color.green : Color.secondary).frame(width: 8, height: 8)
                    Text(exists ? "HuggingFace キャッシュに配置済みです" : "HuggingFace キャッシュに未ダウンロードです")
                        .font(.system(size: 11))
                        .foregroundStyle(exists ? Color.green : Color.secondary)
                    Spacer()
                    if exists {
                        Button("このモデルを適用") {
                            let url = HuggingFaceHub.modelURL(repo: model.repoPath, filename: model.modelFilename)
                            model.modelPath = url.path
                            model.loadModel()
                            model.saveTauriConfig()
                        }
                        .buttonStyle(.borderedProminent)
                        .controlSize(.small)
                    }
                    Button("キャッシュフォルダを開く") {
                        model.openModelDirectory()
                    }
                    .buttonStyle(.bordered)
                    .controlSize(.small)
                }
            }

            Divider()

            // Direct local GGUF file path
            VStack(alignment: .leading, spacing: 6) {
                Text("現在ロード対象の GGUF ファイルパス").font(.system(size: 12, weight: .medium))
                HStack(spacing: 8) {
                    TextField("ローカル GGUF パス", text: $model.modelPath)
                        .textFieldStyle(.roundedBorder)
                    Button("選択…") { model.selectModel() }
                    Button(model.isLoadingModel ? "読み込み中…" : "読み込む") { model.loadModel() }
                        .disabled(model.modelPath.isEmpty || model.isLoadingModel)
                }
                Text(model.modelStatus)
                    .font(.system(size: 11))
                    .foregroundStyle(model.modelStatus.hasPrefix("エラー") ? .red : .secondary)
            }

            Divider()

            // Advanced context & generation parameters
            VStack(alignment: .leading, spacing: 12) {
                Text("詳細コンテキスト & 生成パラメータ").font(.system(size: 13, weight: .medium))

                HStack(spacing: 14) {
                    VStack(alignment: .leading, spacing: 4) {
                        Text("コンテキスト長 (n_ctx)").font(.system(size: 11))
                        TextField("8192", value: Binding(
                            get: { model.settings.nCtx },
                            set: { model.settings.nCtx = $0; model.saveTauriConfig() }
                        ), formatter: NumberFormatter())
                        .textFieldStyle(.roundedBorder)
                        .frame(width: 100)
                    }

                    VStack(alignment: .leading, spacing: 4) {
                        Text("最大生成トークン (max_gen)").font(.system(size: 11))
                        TextField("2048", value: Binding(
                            get: { model.settings.maxGen },
                            set: { model.settings.maxGen = $0; model.saveTauriConfig() }
                        ), formatter: NumberFormatter())
                        .textFieldStyle(.roundedBorder)
                        .frame(width: 100)
                    }

                    VStack(alignment: .leading, spacing: 4) {
                        Text("保持トークン数 (prompt_keep)").font(.system(size: 11))
                        TextField("500", value: Binding(
                            get: { model.settings.promptKeepTokens },
                            set: { model.settings.promptKeepTokens = $0; model.saveTauriConfig() }
                        ), formatter: NumberFormatter())
                        .textFieldStyle(.roundedBorder)
                        .frame(width: 100)
                    }
                }
            }

            Divider()

            // KV Cache Preload options
            VStack(alignment: .leading, spacing: 8) {
                Text("ワーカー別 KV キャッシュ・プリロード").font(.system(size: 13, weight: .medium))
                Text("モデルロード時に各専門ワーカーのシステムプロンプトを KV キャッシュに事前展開します。").font(.system(size: 11)).foregroundStyle(.secondary)

                LazyVGrid(columns: [GridItem(.flexible()), GridItem(.flexible())], spacing: 10) {
                    Toggle("ナレッジワーカー", isOn: Binding(
                        get: { model.settings.preloadKnowledge },
                        set: { model.settings.preloadKnowledge = $0; model.saveTauriConfig() }
                    ))
                    Toggle("アナリストワーカー", isOn: Binding(
                        get: { model.settings.preloadAnalysis },
                        set: { model.settings.preloadAnalysis = $0; model.saveTauriConfig() }
                    ))
                    Toggle("RAG ワーカー", isOn: Binding(
                        get: { model.settings.preloadRag },
                        set: { model.settings.preloadRag = $0; model.saveTauriConfig() }
                    ))
                    Toggle("ビルダーワーカー", isOn: Binding(
                        get: { model.settings.preloadBuilder },
                        set: { model.settings.preloadBuilder = $0; model.saveTauriConfig() }
                    ))
                    Toggle("プロッターワーカー", isOn: Binding(
                        get: { model.settings.preloadPlotter },
                        set: { model.settings.preloadPlotter = $0; model.saveTauriConfig() }
                    ))
                    Toggle("要約ワーカー", isOn: Binding(
                        get: { model.settings.preloadSummarization },
                        set: { model.settings.preloadSummarization = $0; model.saveTauriConfig() }
                    ))
                }
            }
        }
    }

    // MARK: Category 2: Vision Settings

    private var visionSection: some View {
        VStack(alignment: .leading, spacing: 18) {
            Text("Vision (画像・マルチモーダル) 設定").font(.system(size: 15, weight: .semibold))

            Toggle(isOn: Binding(
                get: { model.settings.visionEnabled },
                set: { model.settings.visionEnabled = $0; model.saveTauriConfig() }
            )) {
                VStack(alignment: .leading, spacing: 2) {
                    Text("Vision 機能を有効化").font(.system(size: 13, weight: .medium))
                    Text("トポロジ図や機器外観の画像を読み取って分析するマルチモーダル機能を有効化します。").font(.system(size: 11)).foregroundStyle(.secondary)
                }
            }

            Divider()

            VStack(alignment: .leading, spacing: 6) {
                Text("マルチモーダルプロジェクター (mmproj)").font(.system(size: 12, weight: .medium))
                Text("GGUF マルチモーダルプロジェクターファイル (例: mmproj-F16.gguf) のパスを指定します。").font(.system(size: 11)).foregroundStyle(.secondary)
                HStack(spacing: 8) {
                    TextField("mmproj ファイルパス", text: Binding(
                        get: { model.settings.mmprojPath ?? "" },
                        set: { model.settings.mmprojPath = $0.isEmpty ? nil : $0; model.saveTauriConfig() }
                    ))
                    .textFieldStyle(.roundedBorder)

                    Button("選択…") {
                        let panel = NSOpenPanel()
                        if let gguf = UTType(filenameExtension: "gguf") { panel.allowedContentTypes = [gguf] }
                        panel.canChooseFiles = true
                        panel.canChooseDirectories = false
                        panel.allowsMultipleSelection = false
                        if panel.runModal() == .OK, let url = panel.url {
                            model.settings.mmprojPath = url.path
                            model.saveTauriConfig()
                        }
                    }
                }
            }
        }
    }

    // MARK: Category 3: Knowledge Base RAG

    private var knowledgeSection: some View {
        VStack(alignment: .leading, spacing: 18) {
            Text("ナレッジベース (RAG) 設定").font(.system(size: 15, weight: .semibold))

            VStack(alignment: .leading, spacing: 6) {
                Text("埋め込みモデル").font(.system(size: 12, weight: .medium))
                HStack {
                    Text("MultilingualE5Large (多言語対応ベクトル埋め込み)")
                        .font(.system(size: 12, design: .monospaced))
                    Spacer()
                    Text("固定").font(.system(size: 11)).foregroundStyle(.secondary)
                }
                .padding(10)
                .background(Color(nsColor: .controlBackgroundColor), in: RoundedRectangle(cornerRadius: 6))
            }

            Divider()

            FolderPickerRow(title: "技術資料フォルダ (Markdown コーパス)", path: $model.documentsDirectory)
            Text("Cisco、Yamaha、Fitelnet などの Markdown ドキュメントが配置されたディレクトリです。").font(.system(size: 11)).foregroundStyle(.secondary)

            Divider()

            FolderPickerRow(title: "検索インデックス保存先", path: $model.knowledgeDirectory)
            Text("ベクトルインデックスやメタデータキャッシュが保存されるローカルディレクトリです。").font(.system(size: 11)).foregroundStyle(.secondary)
        }
    }

    // MARK: Category 4: Tauri Config Synchronization

    private var tauriSyncSection: some View {
        VStack(alignment: .leading, spacing: 18) {
            Text("Tauri 版設定との自動同期").font(.system(size: 15, weight: .semibold))

            VStack(alignment: .leading, spacing: 10) {
                HStack(spacing: 8) {
                    Circle().fill(model.isTauriConfigLoaded ? Color.green : Color.orange).frame(width: 10, height: 10)
                    Text(model.isTauriConfigLoaded ? "Tauri 版設定と同期中" : "デフォルト設定を使用中 (Tauri設定未検出)")
                        .font(.system(size: 13, weight: .semibold))
                        .foregroundStyle(model.isTauriConfigLoaded ? Color.green : Color.orange)
                    Spacer()
                }

                Text(model.tauriSyncMessage)
                    .font(.system(size: 12, design: .monospaced))
                    .foregroundStyle(.secondary)
                    .textSelection(.enabled)
                    .padding(10)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(Color(nsColor: .controlBackgroundColor), in: RoundedRectangle(cornerRadius: 6))

                HStack(spacing: 10) {
                    Button("Tauri 設定を再読込") {
                        model.loadTauriConfig()
                    }
                    .buttonStyle(.bordered)

                    Button("Tauri 設定へ保存") {
                        model.saveTauriConfig()
                    }
                    .buttonStyle(.borderedProminent)

                    Button("設定フォルダを Finder で開く") {
                        model.openTauriConfigDirectory()
                    }
                    .buttonStyle(.bordered)

                    Spacer()

                    Button(role: .destructive, action: model.resetSettingsToDefault) {
                        Text("デフォルトにリセット")
                    }
                    .buttonStyle(.borderless)
                    .foregroundStyle(.red)
                }
            }

            Divider()

            VStack(alignment: .leading, spacing: 8) {
                Text("同期される項目一覧").font(.system(size: 13, weight: .medium))
                Text("以下の全項目が Tauri 版 (`settings.json`) と本ネイティブアプリ間で相互に共有されます:")
                    .font(.system(size: 11)).foregroundStyle(.secondary)

                VStack(alignment: .leading, spacing: 4) {
                    Text("• 会話履歴数 (`historyLimit`), サンプリング温度 (`temperature`), 繰り返しペナルティ (`repetitionPenalty`)")
                    Text("• モデルパス (`modelPath`), リポジトリ名, GGUFファイル名")
                    Text("• コンテキスト長 (`nCtx`), 最大生成数 (`maxGen`), プロンプト保持数 (`promptKeepTokens`)")
                    Text("• MCP タイムアウト (`mcpTimeout`), キャッシュ保持時間 (`cacheExpiryMinutes`), IP設定 (`ipVersion`)")
                    Text("• 自動 Dry-Run (`autoDryRun`), シリアルポート (`consolePort`), ボーレート (`consoleBaudRate`)")
                    Text("• 6種のワーカー別 KV プリロード (`preloadKnowledge`, `preloadAnalysis` など)")
                    Text("• Vision 有効化 (`visionEnabled`) およびプロジェクターパス (`mmprojPath`)")
                }
                .font(.system(size: 11, design: .monospaced))
                .foregroundStyle(.secondary)
                .padding(12)
                .background(Color(nsColor: .controlBackgroundColor).opacity(0.6), in: RoundedRectangle(cornerRadius: 6))
            }
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
