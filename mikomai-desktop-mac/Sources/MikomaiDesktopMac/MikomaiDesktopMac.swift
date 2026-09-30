import SwiftUI
import AppKit
import Darwin
import Security
import CryptoKit
import MikomaiFFI
import MikomaiDesktopCore
import UniformTypeIdentifiers

// SwiftPM launches an unbundled executable. Explicitly register it as a
// foreground app so its windows can receive keyboard and IME events.
private final class DesktopAppDelegate: NSObject, NSApplicationDelegate {
    func applicationWillFinishLaunching(_ notification: Notification) {
        NSApplication.shared.setActivationPolicy(.regular)
        if let iconURL = Bundle.module.url(forResource: "AppIcon", withExtension: "icns"),
           let icon = NSImage(contentsOf: iconURL) {
            NSApplication.shared.applicationIconImage = icon
        }
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
                Button("監視・タスク履歴") { model.workspace = .monitoring }.keyboardShortcut("5", modifiers: .command)
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
    case monitoring = "監視・タスク履歴"
    case settings = "設定"

    var id: String { rawValue }
    var icon: String {
        switch self {
        case .chat: "bubble.left.and.bubble.right"
        case .connections: "point.3.connected.trianglepath.dotted"
        case .tools: "wrench.and.screwdriver"
        case .monitoring: "waveform.path.ecg"
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

// MARK: - Native Settings Model

private typealias AppSettings = DesktopSettings

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

// MARK: - Native Settings Store

private enum SettingsManager {
    static var settingsURL: URL {
        if let env = ProcessInfo.processInfo.environment["MIKOMAI_SETTINGS_PATH"], !env.isEmpty {
            return URL(fileURLWithPath: env)
        }
        let support = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first
            ?? FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Library/Application Support")
        return support.appendingPathComponent("MikomaiDesktopMac/settings.json")
    }

    static func load() -> (settings: AppSettings, url: URL, source: String?) {
        let url = settingsURL
        if let data = try? Data(contentsOf: url), let decoded = try? DesktopSettingsCodec.decode(data) {
            return (decoded, url, "native")
        }

        // Import existing installs once; all future saves go to the native app store.
        let support = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first
        let legacyURLs = [
            support?.appendingPathComponent("com.mikomai.agent/settings.json"),
            support?.appendingPathComponent("mikomai/settings.json"),
            FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".config/mikomai/settings.json")
        ].compactMap { $0 }.filter { $0.standardizedFileURL != url.standardizedFileURL }
        for legacyURL in legacyURLs {
            guard let data = try? Data(contentsOf: legacyURL),
                  let decoded = try? DesktopSettingsCodec.decode(data) else { continue }
            try? save(decoded)
            return (decoded, url, "imported")
        }
        return (AppSettings(), url, nil)
    }

    static func save(_ settings: AppSettings) throws {
        let url = settingsURL
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

private struct NativeOperationPlan: Decodable, Identifiable {
    let id: String
    let planHash: String
    let status: String
    let toolId: String
    let rationale: String
    var target: String?
    let args: NativeOperationPlanArgs
}

private struct NativeOperationPlanArgs: Decodable {
    let commands: [String]?
    let deviceSnapshot: NativeDeviceSnapshot
}

private struct NativeDeviceSnapshot: Codable, Equatable {
    let id: String
    let name: String
    let host: String
    let username: String
    let deviceType: String
    let connectionType: String
    let port: String
    let credentialsFingerprint: String

    init(_ connection: SavedConnection, credentials: ConnectionCredentials) {
        id = connection.id.uuidString
        name = connection.name
        host = connection.host
        username = connection.username
        deviceType = connection.deviceType
        connectionType = connection.connectionType ?? "SSH"
        port = connection.port
        let credentialText = "\(credentials.password ?? "")\u{0}\(credentials.enablePassword ?? "")"
        credentialsFingerprint = SHA256.hash(data: Data(credentialText.utf8)).map { String(format: "%02x", $0) }.joined()
    }
}

private struct NetworkRunnerRequest: Sendable {
    let action: String
    let host: String
    let username: String
    let password: String
    let secret: String
    let deviceType: String
    let port: String
    let commands: [String]
}

private struct NetworkOperationOutput: Sendable {
    let success: Bool
    let stdout: String
    let stderr: String
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

private final class ChatCallbackBox: @unchecked Sendable {
    let stream: StreamBox
    let connections: [SavedConnection]
    let credentialPersistence: ConnectionCredentialPersistence
    let onOperationPlan: (Data) -> Void

    init(stream: StreamBox, connections: [SavedConnection], credentialPersistence: ConnectionCredentialPersistence, onOperationPlan: @escaping (Data) -> Void) {
        self.stream = stream
        self.connections = connections
        self.credentialPersistence = credentialPersistence
        self.onOperationPlan = onOperationPlan
    }
}

private func streamBridge(chunk: UnsafePointer<CChar>?, isDone: Int32, context: UnsafeMutableRawPointer?) {
    guard let context else { return }
    let box = Unmanaged<ChatCallbackBox>.fromOpaque(context).takeUnretainedValue()
    let text = chunk.flatMap { String(cString: $0) } ?? ""
    let approvalPrefix = "__MIKOMAI_APPROVAL_PLAN__"
    if text.hasPrefix(approvalPrefix) {
        let json = String(text.dropFirst(approvalPrefix.count)).components(separatedBy: "\n").first ?? ""
        box.onOperationPlan(Data(json.utf8))
        return
    }
    box.stream.onChunk(text, isDone != 0)
}

private struct PortableDeviceTarget: Decodable {
    let id: String?
    let hostname: String
    let ip: String?
    let deviceType: String?
}

private struct NativeWatch: Codable, Identifiable {
    struct IR: Codable {
        struct Schedule: Codable { var every: String }
        struct CallArgs: Codable { var device: String; var resource: String }
        struct Call: Codable { var id: String; var call: String; var args: CallArgs }
        struct Reference: Codable { var ref: String }
        struct Comparison: Codable { var left: Reference; var `operator`: String; var right: Double }
        struct NotificationArgs: Codable { var message: String }
        struct Notification: Codable { var call: String; var args: NotificationArgs }
        struct When: Codable { var when: Comparison; var then: [Notification] }
        var version: Int
        var schedule: Schedule
        var steps: [Step]
        enum Step: Codable {
            case call(Call)
            case when(When)
            init(from decoder: Decoder) throws {
                let container = try decoder.singleValueContainer()
                if let call = try? container.decode(Call.self) { self = .call(call); return }
                self = .when(try container.decode(When.self))
            }
            func encode(to encoder: Encoder) throws {
                var container = encoder.singleValueContainer()
                switch self { case .call(let value): try container.encode(value); case .when(let value): try container.encode(value) }
            }
        }
    }
    struct Run: Codable, Identifiable {
        struct Notice: Codable, Identifiable { var watchId: String; var message: String; var emittedAt: String; var id: String { emittedAt } }
        var runId: String; var startedAt: String; var completedAt: String; var notifications: [Notice]; var error: String?
        var id: String { runId }
    }
    var id: String
    var name: String
    var status: String
    var ir: IR
    var createdAt: String
    var lastRunAt: String?
    var lastError: String?
    var history: [Run]?
}

private struct NativeAgentTask: Decodable, Identifiable {
    var taskId: String
    var goal: String
    var status: String
    var startedAt: String
    var lastEventAt: String
    var eventCount: Int
    var id: String { taskId }
}

private struct WatchAlert: Identifiable {
    let id = UUID()
    let message: String
}

private final class WatchCallbackBox: @unchecked Sendable {
    private let lock = NSLock()
    private var savedConnections: [SavedConnection]
    let credentialPersistence: ConnectionCredentialPersistence
    let onNotification: @Sendable (Data) -> Void
    init(connections: [SavedConnection], credentialPersistence: ConnectionCredentialPersistence, onNotification: @escaping @Sendable (Data) -> Void) {
        self.savedConnections = connections
        self.credentialPersistence = credentialPersistence
        self.onNotification = onNotification
    }
    var connections: [SavedConnection] { lock.lock(); defer { lock.unlock() }; return savedConnections }
    func update(connections: [SavedConnection]) { lock.lock(); savedConnections = connections; lock.unlock() }
}

private func watchToolBridge(
    toolID: UnsafePointer<CChar>?, targetJSON: UnsafePointer<CChar>?, argsJSON: UnsafePointer<CChar>?,
    output: UnsafeMutablePointer<CChar>?, outputCapacity: UInt, context: UnsafeMutableRawPointer?
) -> Int32 {
    guard let output, outputCapacity > 0 else { return 1 }
    let capacity = Int(outputCapacity); output[0] = 0
    guard let context, let toolID, let targetJSON, let argsJSON else { return 1 }
    let box = Unmanaged<WatchCallbackBox>.fromOpaque(context).takeUnretainedValue()
    do {
        let target = try JSONDecoder().decode(PortableDeviceTarget.self, from: Data(String(cString: targetJSON).utf8))
        let arguments = try JSONSerialization.jsonObject(with: Data(String(cString: argsJSON).utf8)) as? [String: Any] ?? [:]
        let result = DesktopModel.runPortableAgentTool(tool: String(cString: toolID), target: target, arguments: arguments, connections: box.connections, credentialPersistence: box.credentialPersistence)
        let payload = try JSONSerialization.data(withJSONObject: ["success": result.success, "output": result.success ? result.stdout : result.stderr])
        let text = String(decoding: payload, as: UTF8.self)
        return text.withCString { strlcpy(output, $0, capacity) < capacity ? 0 : 1 }
    } catch {
        let text = "watch probe failed: \(error.localizedDescription)"
        return text.withCString { _ = strlcpy(output, $0, capacity); return 1 }
    }
}

private func watchNotificationBridge(notificationJSON: UnsafePointer<CChar>?, context: UnsafeMutableRawPointer?) {
    guard let context, let notificationJSON else { return }
    let box = Unmanaged<WatchCallbackBox>.fromOpaque(context).takeUnretainedValue()
    box.onNotification(Data(String(cString: notificationJSON).utf8))
}

private func agentToolBridge(
    toolID: UnsafePointer<CChar>?,
    targetJSON: UnsafePointer<CChar>?,
    argsJSON: UnsafePointer<CChar>?,
    output: UnsafeMutablePointer<CChar>?,
    outputCapacity: UInt,
    context: UnsafeMutableRawPointer?
) -> Int32 {
    guard let output, outputCapacity > 0 else { return 1 }
    let capacity = Int(outputCapacity)
    output[0] = 0
    guard let context, let toolID, let targetJSON, let argsJSON else {
        "agent tool bridge arguments are missing".withCString { _ = strlcpy(output, $0, capacity) }
        return 1
    }
    let box = Unmanaged<ChatCallbackBox>.fromOpaque(context).takeUnretainedValue()
    let tool = String(cString: toolID)
    do {
        let target = try JSONDecoder().decode(PortableDeviceTarget.self, from: Data(String(cString: targetJSON).utf8))
        let arguments = try JSONSerialization.jsonObject(with: Data(String(cString: argsJSON).utf8)) as? [String: Any] ?? [:]
        let result = DesktopModel.runPortableAgentTool(
            tool: tool,
            target: target,
            arguments: arguments,
            connections: box.connections,
            credentialPersistence: box.credentialPersistence
        )
        let payload = try JSONSerialization.data(withJSONObject: ["success": result.success, "output": result.success ? result.stdout : result.stderr])
        let text = String(decoding: payload, as: UTF8.self)
        let copied = text.withCString { strlcpy(output, $0, capacity) }
        return copied < capacity ? 0 : 1
    } catch {
        let text = "agent tool failed: \(error.localizedDescription)"
        let _ = text.withCString { strlcpy(output, $0, capacity) }
        return 1
    }
}

private func agentPlanBridge(
    target: UnsafePointer<CChar>?,
    toolID: UnsafePointer<CChar>?,
    argsJSON: UnsafePointer<CChar>?,
    rationale: UnsafePointer<CChar>?,
    output: UnsafeMutablePointer<CChar>?,
    outputCapacity: UInt,
    context: UnsafeMutableRawPointer?
) -> Int32 {
    guard let output, outputCapacity > 0 else { return 1 }
    let capacity = Int(outputCapacity)
    output[0] = 0
    guard let context, let target, let toolID, let argsJSON, let rationale else { return 1 }
    let box = Unmanaged<ChatCallbackBox>.fromOpaque(context).takeUnretainedValue()
    let targetName = String(cString: target)
    guard let connection = box.connections.first(where: { $0.name == targetName || $0.host == targetName || $0.id.uuidString == targetName }) else {
        "変更対象がSwift側の登録端末にありません。".withCString { _ = strlcpy(output, $0, capacity) }
        return 1
    }
    guard let credentialsJSON = try? String(data: JSONEncoder().encode(NativeDeviceSnapshot(connection, credentials: box.credentialPersistence.load(for: connection.id))), encoding: .utf8) else { return 1 }
    let toolName = String(cString: toolID)
    let args = String(cString: argsJSON)
    let rationaleText = String(cString: rationale)
    let response = connection.name.withCString { targetPtr in
        credentialsJSON.withCString { snapshotPtr in
            toolName.withCString { toolPtr in
                args.withCString { argsPtr in
                rationaleText.withCString { rationalePtr in
                    mikomai_operation_plan_create_generic(targetPtr, toolPtr, snapshotPtr, argsPtr, rationalePtr)
                }
                }
            }
        }
    }
    defer { mikomai_result_free(response) }
    guard response.status == 0, let message = response.message else {
        let text = response.message.map { String(cString: $0) } ?? "変更計画を作成できませんでした。"
        let _ = text.withCString { strlcpy(output, $0, capacity) }
        return 1
    }
    let copied = strlcpy(output, message, capacity)
    return copied < capacity ? 0 : 1
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
    @Published var connections: [SavedConnection] = [] { didSet { persistConnections(); watchCallbackBox?.update(connections: connections) } }
    @Published var editingConnection: SavedConnection?
    @Published var connectionStatuses: [UUID: ConnectionTestStatus] = [:]
    @Published var operationProposal = ""
    @Published var operationPlan: NativeOperationPlan?
    @Published var operationLogs: [String] = []
    @Published var operationBeforeConfig = ""
    @Published var operationAfterConfig = ""
    @Published var operationDiffLines: [String] = []
    @Published var operationPhase = "idle"
    @Published var watches: [NativeWatch] = []
    @Published var watchStatus = "監視サービス未起動"
    @Published var watchAlert: WatchAlert?
    @Published var watchDevice = ""
    @Published var watchName = "CPU 使用率"
    @Published var watchInterval = "60"
    @Published var watchThreshold = "80"
    @Published var watchMessage = "CPU 使用率がしきい値を超えました"
    @Published var watchEditingID: String?
    @Published var agentTasks: [NativeAgentTask] = []
    @Published var selectedTaskHistory = ""
    @Published var operationAuditText = ""
    @Published var selectedAgentTaskID: String?
    private var watchCallbackBox: WatchCallbackBox?
    private var watchCallbackContext: UnsafeMutableRawPointer?
    private var pendingAgentTaskIDs: [UUID: String] = [:]
    private var pendingSavedAgentTaskIDs: [UUID: String] = [:]

    // Knowledge dirs
    @Published var documentsDirectory: String { didSet { defaults.set(documentsDirectory, forKey: "mikomai.desktop.mac.documentsDirectory") } }
    @Published var knowledgeDirectory: String { didSet { defaults.set(knowledgeDirectory, forKey: "mikomai.desktop.mac.knowledgeDirectory") } }

    // Model path & status
    @Published var modelPath: String = "" { didSet { defaults.set(modelPath, forKey: "mikomai.desktop.mac.modelPath") } }
    @Published var modelStatus = "モデル未ロード"
    @Published var isLoadingModel = false
    @Published var isCancelling = false

    // Native settings
    @Published var settings: AppSettings = AppSettings()
    @Published var settingsFileURL: URL = SettingsManager.settingsURL
    @Published var registryHosts: [HostSuggestion] = []
    private var completionReloadTask: Task<Void, Never>?
    private var lastCompletionReload = Date.distantPast
    @Published var isSettingsLoaded: Bool = false
    @Published var settingsStatusMessage: String = ""

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

        loadSettings()

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

    func startWatchService() {
        guard watchCallbackContext == nil else { refreshWatches(); return }
        let fm = FileManager.default
        let support = (fm.urls(for: .applicationSupportDirectory, in: .userDomainMask).first ?? fm.temporaryDirectory)
            .appendingPathComponent("MikomaiDesktopMac", isDirectory: true)
        let destination = support.appendingPathComponent("watches.json")
        let legacy = fm.homeDirectoryForCurrentUser.appendingPathComponent("Library/Application Support/com.mikomai.agent/watches.json")
        do {
            try fm.createDirectory(at: support, withIntermediateDirectories: true)
            if !fm.fileExists(atPath: destination.path), fm.fileExists(atPath: legacy.path) {
                try fm.copyItem(at: legacy, to: destination)
                watchStatus = "旧版の監視設定をSwift版へ移行しました"
            }
        } catch {
            watchStatus = "監視設定の移行に失敗しました: \(error.localizedDescription)"
            return
        }
        let box = WatchCallbackBox(connections: connections, credentialPersistence: credentialPersistence) { [weak self] data in
            Task { @MainActor [weak self] in
                guard let self, let value = try? JSONDecoder().decode(NativeWatch.Run.Notice.self, from: data) else { return }
                self.watchStatus = value.message
                self.watchAlert = WatchAlert(message: value.message)
                NSSound.beep()
                self.refreshWatches()
            }
        }
        let context = Unmanaged.passRetained(box).toOpaque()
        let response = destination.path.withCString { mikomai_watch_start($0, watchToolBridge, watchNotificationBridge, context) }
        defer { mikomai_result_free(response) }
        if response.status == 0 {
            watchCallbackBox = box
            watchCallbackContext = context
            if watchStatus == "監視サービス未起動" { watchStatus = "定期監視を実行中" }
            refreshWatches()
        } else {
            Unmanaged<WatchCallbackBox>.fromOpaque(context).release()
            watchStatus = response.message.map { String(cString: $0) } ?? "監視サービスを開始できませんでした"
        }
        refreshAgentTasks()
    }

    func stopWatchService() {
        guard let context = watchCallbackContext else { return }
        let response = mikomai_watch_stop()
        let succeeded = response.status == 0
        let message = response.message.map { String(cString: $0) }
        mikomai_result_free(response)
        guard succeeded else { watchStatus = message ?? "監視サービスの停止に失敗しました"; return }
        watchCallbackContext = nil
        watchCallbackBox = nil
        Unmanaged<WatchCallbackBox>.fromOpaque(context).release()
        watchStatus = message ?? "監視サービスを停止しました"
    }

    func refreshWatches() {
        let response = mikomai_watch_list()
        defer { mikomai_result_free(response) }
        guard response.status == 0, let text = response.message,
              let decoded = try? JSONDecoder().decode([NativeWatch].self, from: Data(String(cString: text).utf8)) else { return }
        watches = decoded
    }

    func createCPUWatch() {
        if watchCallbackContext == nil {
            startWatchService()
            guard watchCallbackContext != nil else { return }
        }
        guard !watchDevice.isEmpty, let interval = Int(watchInterval), interval > 0,
              let threshold = Double(watchThreshold), (0...100).contains(threshold) else {
            watchStatus = "機器、正の監視間隔、0〜100のしきい値を指定してください"; return
        }
        let ir: [String: Any] = [
            "version": 1, "schedule": ["every": "\(interval)s"],
            "steps": [
                ["id": "cpu", "call": "get_state", "args": ["device": watchDevice, "resource": "cpu"]],
                ["when": ["left": ["ref": "cpu.usage"], "operator": "gt", "right": threshold],
                 "then": [["call": "notify", "args": ["message": watchMessage]]]]
            ]
        ]
        guard let data = try? JSONSerialization.data(withJSONObject: ["name": watchName, "ir": ir]) else { return }
        let payload = String(decoding: data, as: UTF8.self)
        let response: MikomaiResult
        if let editingID = watchEditingID {
            response = editingID.withCString { id in payload.withCString { mikomai_watch_update(id, $0) } }
        } else {
            response = payload.withCString { mikomai_watch_create($0) }
        }
        let message = response.message.map { String(cString: $0) } ?? "監視設定を作成できませんでした"
        let ok = response.status == 0
        mikomai_result_free(response)
        watchStatus = ok ? (watchEditingID == nil ? "監視設定を作成しました" : "監視設定を更新しました") : message
        if ok { watchEditingID = nil }
        refreshWatches()
    }

    func editWatch(_ watch: NativeWatch) {
        watchEditingID = watch.id
        watchName = watch.name
        watchInterval = String(watch.ir.schedule.every.dropLast())
        for step in watch.ir.steps {
            switch step {
            case .call(let call): watchDevice = call.args.device
            case .when(let condition):
                watchThreshold = String(condition.when.right)
                if let notification = condition.then.first { watchMessage = notification.args.message }
            }
        }
    }

    func setWatch(_ watch: NativeWatch, enabled: Bool) {
        let response = watch.id.withCString { enabled ? mikomai_watch_enable($0) : mikomai_watch_disable($0) }
        let message = response.message.map { String(cString: $0) } ?? "更新できませんでした"
        watchStatus = response.status == 0 ? (enabled ? "監視を有効にしました" : "監視を停止しました") : message
        mikomai_result_free(response); refreshWatches()
    }

    func runWatch(_ watch: NativeWatch) {
        let response = watch.id.withCString { mikomai_watch_run_now($0) }
        watchStatus = Self.consumeRust(response)
        refreshWatches()
    }

    func deleteWatch(_ watch: NativeWatch) {
        let response = watch.id.withCString { mikomai_watch_delete($0) }
        watchStatus = Self.consumeRust(response); refreshWatches()
    }

    func refreshAgentTasks() {
        let response = mikomai_agent_task_list()
        defer { mikomai_result_free(response) }
        guard response.status == 0, let text = response.message,
              let decoded = try? JSONDecoder().decode([NativeAgentTask].self, from: Data(String(cString: text).utf8)) else { return }
        agentTasks = decoded
    }

    func refreshOperationAudit() {
        let response = mikomai_operation_audit_list()
        defer { mikomai_result_free(response) }
        guard response.status == 0, let message = response.message else {
            operationAuditText = response.message.map { String(cString: $0) } ?? "操作監査記録を読み込めませんでした"
            return
        }
        let raw = String(cString: message)
        if let data = raw.data(using: .utf8), let value = try? JSONSerialization.jsonObject(with: data),
           let formatted = try? JSONSerialization.data(withJSONObject: value, options: [.prettyPrinted, .sortedKeys]) {
            operationAuditText = String(decoding: formatted, as: UTF8.self)
        } else { operationAuditText = raw }
    }

    func loadAgentTaskHistory(_ task: NativeAgentTask) {
        selectedAgentTaskID = task.id
        let response = task.id.withCString { mikomai_agent_task_history($0) }
        defer { mikomai_result_free(response) }
        selectedTaskHistory = response.message.map { String(cString: $0) } ?? "タスク履歴を読み込めませんでした"
    }

    func resumeAgentTask(_ task: NativeAgentTask) {
        createSession()
        if let activeSessionID { pendingSavedAgentTaskIDs[activeSessionID] = task.id }
        draft = "以前の調査結果を踏まえて、続きから対応してください。"
        send()
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

    var availableCompletionHosts: [HostSuggestion] {
        HostCompletionSource.merge(
            registry: registryHosts,
            native: connections.map { HostSuggestion(hostname: $0.name, ip: $0.host) }
        )
    }

    func reloadCompletionHosts() {
        guard completionReloadTask == nil, Date().timeIntervalSince(lastCompletionReload) > 1 else { return }
        lastCompletionReload = Date()
        let override = ProcessInfo.processInfo.environment["MIKOMAI_CONNECTIONS_FILE"]
        let path = override.map { URL(fileURLWithPath: $0) }
            ?? settingsFileURL.deletingLastPathComponent().appendingPathComponent("connections.json")
        completionReloadTask = Task {
            let hosts = await Task.detached(priority: .utility) {
                HostCompletionSource.read(from: path)
            }.value
            registryHosts = hosts
            completionReloadTask = nil
        }
    }

    // MARK: - Native Settings Management

    func loadSettings() {
        let (loadedSettings, url, source) = SettingsManager.load()
        self.settings = loadedSettings
        self.settingsFileURL = url
        self.isSettingsLoaded = source != nil

        if source != nil {
            self.settingsStatusMessage = source == "imported"
                ? "既存設定を読み込み、Swift版の保存先へ移行しました: \(url.path)"
                : "Swift版設定を読み込みました: \(url.path)"
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
            self.settingsStatusMessage = "Swift版設定ファイルがありません。デフォルト値を使用しています: \(url.path)"
            if !self.modelPath.isEmpty && FileManager.default.fileExists(atPath: self.modelPath) {
                loadModel()
            }
        }

        applyInferenceParams()
    }

    func saveSettings() {
        var toSave = settings
        if !modelPath.isEmpty {
            var patch = DesktopSettingsPatch()
            patch.modelPath = .set(modelPath)
            toSave.merge(patch)
        }
        do {
            try SettingsManager.save(toSave)
            self.isSettingsLoaded = true
            self.settingsStatusMessage = "Swift版設定を保存しました: \(settingsFileURL.path)"
            applyInferenceParams()
        } catch {
            self.settingsStatusMessage = "設定の保存に失敗しました: \(error.localizedDescription)"
        }
    }

    func resetSettingsToDefault() {
        self.settings = AppSettings()
        saveSettings()
        applyInferenceParams()
        self.settingsStatusMessage = "設定をデフォルト値にリセットしました。"
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
                saveSettings()
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
        let submissionText: String
        if let taskID = pendingSavedAgentTaskIDs.removeValue(forKey: id) {
            submissionText = "__MIKOMAI_RESUME_SAVED__\(taskID)"
        } else if let taskID = pendingAgentTaskIDs.removeValue(forKey: id) {
            submissionText = "__MIKOMAI_RESUME__\(taskID)\n\(userText)"
        } else {
            submissionText = userText
        }
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
        let agentConnections = connections
        let agentCredentialPersistence = credentialPersistence
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
                submissionText,
                history: history,
                documents: documents,
                knowledge: knowledge,
                attachments: attachmentText,
                connections: agentConnections,
                credentialPersistence: agentCredentialPersistence,
                onOperationPlan: { data in
                    Task { @MainActor in
                        guard let plan = try? JSONDecoder().decode(NativeOperationPlan.self, from: data) else { return }
                        self.operationPlan = plan
                        if let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                           var args = object["args"] as? [String: Any] {
                            args.removeValue(forKey: "deviceSnapshot")
                            self.operationProposal = plan.args.commands?.joined(separator: "\n") ?? String(decoding: (try? JSONSerialization.data(withJSONObject: args, options: [.prettyPrinted, .sortedKeys])) ?? Data(), as: UTF8.self)
                        } else {
                            self.operationProposal = plan.args.commands?.joined(separator: "\n") ?? "\(plan.toolId)\n\(plan.rationale)"
                        }
                        self.operationPhase = "エージェント提案を確認中"
                        self.operationLogs = []
                    }
                }
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
                var displayAnswer = finalAnswer
                var receivedChoice = false
                if finalAnswer.hasPrefix("__MIKOMAI_CHOICE__"),
                   let payload = finalAnswer.dropFirst("__MIKOMAI_CHOICE__".count).data(using: .utf8),
                   let choice = try? JSONSerialization.jsonObject(with: payload) as? [String: Any],
                   let taskID = choice["task_id"] as? String,
                   let text = choice["text"] as? String {
                    self.pendingAgentTaskIDs[id] = taskID
                    displayAnswer = text
                    receivedChoice = true
                    if let options = choice["question"] as? [String: Any],
                       let values = options["options"] as? [String], !values.isEmpty {
                        displayAnswer += "\n\n" + values.enumerated().map { "\($0.offset + 1). \($0.element)" }.joined(separator: "\n")
                    }
                }
                if displayAnswer.hasPrefix("エラー:") {
                    if self.sessions[sIdx].messages[mIdx].text.isEmpty {
                        self.sessions[sIdx].messages[mIdx].text = displayAnswer
                    } else {
                        self.sessions[sIdx].messages[mIdx].text += "\n\n[\(displayAnswer)]"
                    }
                } else if receivedChoice || self.sessions[sIdx].messages[mIdx].text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                    self.sessions[sIdx].messages[mIdx].text = displayAnswer
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

    func createOperationPlan(target: SavedConnection, proposal: String, rationale: String) -> String? {
        let commands = proposal.split(whereSeparator: \.isNewline).map(String.init).map { $0.trimmingCharacters(in: .whitespaces) }.filter { !$0.isEmpty }
        guard !commands.isEmpty else { return "変更コマンドがありません。" }
        let args: String
        do { args = String(data: try JSONEncoder().encode(commands), encoding: .utf8) ?? "[]" }
        catch { return error.localizedDescription }
        let snapshotText: String
        let credentials = credentialPersistence.load(for: target.id)
        do { snapshotText = String(data: try JSONEncoder().encode(NativeDeviceSnapshot(target, credentials: credentials)), encoding: .utf8) ?? "{}" }
        catch { return error.localizedDescription }
        let responseText = target.name.withCString { targetPtr in
            snapshotText.withCString { snapshotPtr in
                args.withCString { argsPtr in
                    rationale.withCString { rationalePtr in
                        let response = mikomai_operation_plan_create(targetPtr, snapshotPtr, argsPtr, rationalePtr)
                        defer { mikomai_result_free(response) }
                        guard let message = response.message else { return "エラー: 応答がありません。" }
                        let text = String(cString: message)
                        return response.status == 0 ? text : "エラー: \(text)"
                    }
                }
            }
        }
        guard !responseText.hasPrefix("エラー:") else { return responseText }
        do {
            operationPlan = try JSONDecoder().decode(NativeOperationPlan.self, from: Data(responseText.utf8))
            operationPhase = "計画作成済み"
            operationLogs.append("[STATUS] 変更計画を作成しました（\(target)）")
            return nil
        } catch { return "変更計画を読み取れませんでした: \(error.localizedDescription)" }
    }

    func approveOperationPlan() -> String? {
        guard let plan = operationPlan else { return "承認する変更計画がありません。" }
        let responseText = plan.id.withCString { id in
            plan.planHash.withCString { hash in
                let response = mikomai_operation_plan_approve(id, hash)
                defer { mikomai_result_free(response) }
                guard let message = response.message else { return "エラー: 応答がありません。" }
                let text = String(cString: message)
                return response.status == 0 ? text : "エラー: \(text)"
            }
        }
        guard !responseText.hasPrefix("エラー:") else { return responseText }
        do {
            operationPlan = try JSONDecoder().decode(NativeOperationPlan.self, from: Data(responseText.utf8))
            operationPhase = "承認済み"
            operationLogs.append("[STATUS] ハッシュを照合し、計画を承認しました")
            return nil
        } catch { return "承認状態を読み取れませんでした: \(error.localizedDescription)" }
    }

    func beginOperationPlan() -> String? {
        guard let plan = operationPlan else { return "実行する計画がありません。" }
        let responseText = plan.id.withCString { id in
            plan.planHash.withCString { hash in
                let response = mikomai_operation_plan_begin(id, hash)
                defer { mikomai_result_free(response) }
                guard let message = response.message else { return "エラー: 応答がありません。" }
                let text = String(cString: message)
                return response.status == 0 ? text : "エラー: \(text)"
            }
        }
        guard !responseText.hasPrefix("エラー:") else { return responseText }
        do { operationPlan = try JSONDecoder().decode(NativeOperationPlan.self, from: Data(responseText.utf8)); return nil }
        catch { return "実行状態を読み取れませんでした: \(error.localizedDescription)" }
    }

    func finishOperationPlan(succeeded: Bool) {
        guard let id = operationPlan?.id else { return }
        id.withCString { idPtr in
            let response = mikomai_operation_plan_finish(idPtr, succeeded ? 1 : 0)
            defer { mikomai_result_free(response) }
            guard response.status == 0, let message = response.message,
                  let data = String(cString: message).data(using: .utf8) else { return }
            operationPlan = try? JSONDecoder().decode(NativeOperationPlan.self, from: data)
        }
    }

    func resolveOperationTarget(for plan: NativeOperationPlan) -> (SavedConnection, ConnectionCredentials)? {
        guard let id = UUID(uuidString: plan.args.deviceSnapshot.id),
              let connection = connections.first(where: { $0.id == id }) else { return nil }
        let credentials = credentialPersistence.load(for: id)
        guard NativeDeviceSnapshot(connection, credentials: credentials) == plan.args.deviceSnapshot else { return nil }
        return (connection, credentials)
    }

    func networkRequest(action: String, connection: SavedConnection, commands: [String]) -> NetworkRunnerRequest? {
        guard (connection.connectionType ?? "SSH").lowercased() != "console" else { return nil }
        let credentials = credentialPersistence.load(for: connection.id)
        let deviceType: String = {
            let lower = connection.deviceType.lowercased()
            if lower.contains("juniper") { return "juniper_junos" }
            if lower.contains("nx-os") || lower.contains("nxos") { return "cisco_nxos" }
            if lower.contains("arista") { return "arista_eos" }
            if lower.contains("yamaha") { return "yamaha" }
            if lower.contains("furukawa") || lower.contains("fitel") { return "furukawa_fitelnet" }
            if lower.contains("cisco") { return "cisco_ios" }
            return lower.replacingOccurrences(of: " ", with: "_")
        }()
        return NetworkRunnerRequest(
            action: action, host: connection.host, username: connection.username,
            password: credentials.password ?? "", secret: credentials.enablePassword ?? "",
            deviceType: deviceType, port: connection.port, commands: commands
        )
    }

    nonisolated static func runNetworkWrapper(_ request: NetworkRunnerRequest) -> NetworkOperationOutput {
        let cwd = URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
        let executableResources = Bundle.main.executableURL?.deletingLastPathComponent().appendingPathComponent("../Resources/netmiko_wrapper").standardizedFileURL
        let environmentWrapper = ProcessInfo.processInfo.environment["MIKOMAI_NETMIKO_WRAPPER"].map { URL(fileURLWithPath: $0) }
        let binaries = [
            environmentWrapper,
            Bundle.main.resourceURL?.appendingPathComponent("netmiko_wrapper"),
            executableResources,
            cwd.appendingPathComponent("mikomai-core/assets/bin/netmiko_wrapper-macos-arm64"),
        ].compactMap { $0 }
        let scriptCandidates = [
            Bundle.main.resourceURL?.appendingPathComponent("network/netmiko_wrapper.py"),
            cwd.appendingPathComponent("mikomai-core/assets/network/netmiko_wrapper.py")
        ].compactMap { $0 }
        let script = scriptCandidates.first(where: { FileManager.default.fileExists(atPath: $0.path) })
        let process = Process()
        if let binary = binaries.first(where: { FileManager.default.isExecutableFile(atPath: $0.path) }) {
            process.executableURL = binary
            process.arguments = ["--stdin"]
        } else if let script {
            process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
            process.arguments = ["python3", script.path, "--stdin"]
        } else {
            return NetworkOperationOutput(success: false, stdout: "", stderr: "Netmiko 実行ツールが見つかりません。")
        }

        let input = Pipe()
        let tempDirectory = FileManager.default.temporaryDirectory.appendingPathComponent("mikomai-net-\(UUID().uuidString)", isDirectory: true)
        do { try FileManager.default.createDirectory(at: tempDirectory, withIntermediateDirectories: true) }
        catch { return NetworkOperationOutput(success: false, stdout: "", stderr: "一時ログ領域を作成できません: \(error.localizedDescription)") }
        let outURL = tempDirectory.appendingPathComponent("stdout.log")
        let errURL = tempDirectory.appendingPathComponent("stderr.log")
        FileManager.default.createFile(atPath: outURL.path, contents: nil)
        FileManager.default.createFile(atPath: errURL.path, contents: nil)
        defer { try? FileManager.default.removeItem(at: tempDirectory) }
        guard let out = try? FileHandle(forWritingTo: outURL), let err = try? FileHandle(forWritingTo: errURL) else {
            return NetworkOperationOutput(success: false, stdout: "", stderr: "ログを作成できません。")
        }
        process.standardInput = input
        process.standardOutput = out
        process.standardError = err
        do {
            try process.run()
            let payload: [String: Any] = [
                "action": request.action,
                "host": request.host,
                "username": request.username,
                "password": request.password,
                "secret": request.secret,
                "device_type": request.deviceType,
                "commands": request.commands,
                "port": request.port
            ]
            let data = try JSONSerialization.data(withJSONObject: payload)
            input.fileHandleForWriting.write(data)
            input.fileHandleForWriting.write(Data([0x0a]))
            input.fileHandleForWriting.closeFile()
            process.waitUntilExit()
            try? out.close()
            try? err.close()
            let stdout = (try? String(contentsOf: outURL, encoding: .utf8)) ?? ""
            let stderr = (try? String(contentsOf: errURL, encoding: .utf8)) ?? ""
            return NetworkOperationOutput(success: process.terminationStatus == 0, stdout: stdout, stderr: stderr)
        } catch {
            process.terminate()
            try? out.close(); try? err.close(); input.fileHandleForWriting.closeFile()
            return NetworkOperationOutput(success: false, stdout: "", stderr: error.localizedDescription)
        }
    }

    fileprivate nonisolated static func runPortableAgentTool(
        tool: String,
        target: PortableDeviceTarget,
        arguments: [String: Any],
        connections: [SavedConnection],
        credentialPersistence: ConnectionCredentialPersistence
    ) -> NetworkOperationOutput {
        if tool == "get_state", target.hostname == "localhost", arguments["resource"] as? String == "arp" {
            let process = Process()
            process.executableURL = URL(fileURLWithPath: "/usr/sbin/arp")
            process.arguments = ["-an"]
            let out = Pipe(); let err = Pipe()
            process.standardOutput = out; process.standardError = err
            do {
                try process.run(); process.waitUntilExit()
                let stdout = String(decoding: out.fileHandleForReading.readDataToEndOfFile(), as: UTF8.self)
                let stderr = String(decoding: err.fileHandleForReading.readDataToEndOfFile(), as: UTF8.self)
                return NetworkOperationOutput(success: process.terminationStatus == 0, stdout: stdout, stderr: stderr)
            } catch { return NetworkOperationOutput(success: false, stdout: "", stderr: error.localizedDescription) }
        }
        if tool == "validate_cisco_config" || tool == "convert_cisco_config" {
            guard let script = portableAsset("network/config_helper.py"),
                  let python = portablePython() else {
                return NetworkOperationOutput(success: false, stdout: "", stderr: "Config helperまたはPython runtimeが見つかりません。")
            }
            let payload: [String: Any] = [
                "action": tool == "validate_cisco_config" ? "validate" : "convert",
                "config": arguments["config"] as? String ?? "",
                "target_vendor": arguments["target_vendor"] as? String ?? arguments["targetVendor"] as? String ?? "juniper"
            ]
            return runJSONPython(script: script, python: python, payload: payload)
        }
        if tool == "self_network_nwdiag" {
            guard let wrapper = portableAsset("network/nwdiag_wrapper.py"),
                  let python = portablePython(),
                  let schema = arguments["schema"] as? String ?? arguments["nwdiag"] as? String else {
                return NetworkOperationOutput(success: false, stdout: "", stderr: "nwdiag wrapper、Python runtime、またはschemaがありません。")
            }
            let directory = FileManager.default.temporaryDirectory.appendingPathComponent("mikomai-nwdiag-\(UUID().uuidString)", isDirectory: true)
            let input = directory.appendingPathComponent("network.diag")
            let output = directory.appendingPathComponent("network.svg")
            do {
                try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
                try Data(schema.utf8).write(to: input)
                defer { try? FileManager.default.removeItem(at: directory) }
                let process = Process(); process.executableURL = python
                process.arguments = [wrapper.path, "-T", "svg", "-o", output.path, input.path]
                let stdout = Pipe(); let stderr = Pipe(); process.standardOutput = stdout; process.standardError = stderr
                try process.run(); process.waitUntilExit()
                let err = String(decoding: stderr.fileHandleForReading.readDataToEndOfFile(), as: UTF8.self)
                guard process.terminationStatus == 0, let svg = try? Data(contentsOf: output), !svg.isEmpty else {
                    return NetworkOperationOutput(success: false, stdout: "", stderr: err.isEmpty ? "nwdiag SVG生成に失敗しました。" : err)
                }
                let appSupport = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first ?? FileManager.default.temporaryDirectory
                let artifactDirectory = appSupport.appendingPathComponent("MikomaiDesktopMac/artifacts", isDirectory: true)
                try FileManager.default.createDirectory(at: artifactDirectory, withIntermediateDirectories: true)
                let artifact = artifactDirectory.appendingPathComponent("network-\(UUID().uuidString).svg")
                try svg.write(to: artifact)
                let dataURL = "data:image/svg+xml;base64,\(svg.base64EncodedString())"
                return NetworkOperationOutput(success: true, stdout: "__PORTABLE_ARTIFACT__![Network Diagram](\(dataURL))\n\nSVGを保存しました: \(artifact.path)", stderr: "")
            } catch { return NetworkOperationOutput(success: false, stdout: "", stderr: "nwdiagを実行できませんでした: \(error.localizedDescription)") }
        }
        if ["self_network_ping", "self_network_traceroute", "self_network_test_connection", "self_network_test_net_connection", "self_network_route", "network_get_ip_info", "network_list_serial_ports"].contains(tool) {
            switch tool {
            case "self_network_ping", "self_network_traceroute":
                guard let host = arguments["host"] as? String, !host.isEmpty, host.count <= 255,
                      !host.hasPrefix("-"), host.range(of: "^[A-Za-z0-9._:%-]+$", options: .regularExpression) != nil else {
                    return NetworkOperationOutput(success: false, stdout: "", stderr: "ホスト名またはIPアドレスが不正です。")
                }
                let process = Process()
                if tool == "self_network_ping" {
                    let command = PingCommand(
                        host: host,
                        size: arguments["size"] as? Int,
                        count: arguments["count"] as? Int,
                        df: (arguments["dont_fragment"] as? Bool) ?? (arguments["df"] as? Bool)
                    )
                    guard let commandArguments = command.processArguments else {
                        return NetworkOperationOutput(success: false, stdout: "", stderr: "Pingのサイズまたは引数が範囲外です。")
                    }
                    process.executableURL = URL(fileURLWithPath: "/sbin/ping")
                    process.arguments = commandArguments
                } else {
                    process.executableURL = URL(fileURLWithPath: "/usr/sbin/traceroute")
                    process.arguments = ["-w", "2", "-m", "15", host]
                }
                let out = Pipe(); let err = Pipe()
                process.standardOutput = out; process.standardError = err
                do {
                    try process.run(); process.waitUntilExit()
                    let stdout = String(decoding: out.fileHandleForReading.readDataToEndOfFile(), as: UTF8.self)
                    let stderr = String(decoding: err.fileHandleForReading.readDataToEndOfFile(), as: UTF8.self)
                    return NetworkOperationOutput(success: process.terminationStatus == 0, stdout: stdout, stderr: stderr)
                } catch { return NetworkOperationOutput(success: false, stdout: "", stderr: error.localizedDescription) }
            case "self_network_test_connection", "self_network_test_net_connection":
                guard let host = arguments["host"] as? String, let rawPort = arguments["port"] as? Int,
                      (1...65535).contains(rawPort) else {
                    return NetworkOperationOutput(success: false, stdout: "", stderr: "接続先と有効なportが必要です。")
                }
                let result = testTCP(host: host, port: UInt16(rawPort), timeoutMs: 3000)
                return NetworkOperationOutput(success: result.success, stdout: result.success ? result.message : "", stderr: result.success ? "" : result.message)
            case "self_network_route":
                return runAgentUtility("/sbin/route", ["-n", "get", "default"])
            case "network_get_ip_info":
                return runAgentUtility("/sbin/ifconfig", ["-a"])
            default:
                let ports = SerialPortDetector.listPorts().joined(separator: "\n")
                return NetworkOperationOutput(success: true, stdout: ports.isEmpty ? "シリアルポートは見つかりませんでした。" : ports, stderr: "")
            }
        }
        guard let connection = connections.first(where: {
            $0.id.uuidString == target.id || $0.name == target.hostname || $0.host == target.ip
        }) else {
            return NetworkOperationOutput(success: false, stdout: "", stderr: "登録済み機器が見つかりません。")
        }
        guard (connection.connectionType ?? "SSH").lowercased() != "console" else {
            return NetworkOperationOutput(success: false, stdout: "", stderr: "コンソール接続はこの読み取りエージェントでは未対応です。")
        }
        let resource = arguments["resource"] as? String ?? ""
        let command: String
        switch tool {
        case "network_show":
            guard let supplied = arguments["command"] as? String else {
                return NetworkOperationOutput(success: false, stdout: "", stderr: "show コマンドがありません。")
            }
            command = supplied
        case "fetch_config": command = connection.deviceType.lowercased().contains("yamaha") ? "show config" : "show running-config"
        case "fetch_routing": command = "show ip route"
        case "fetch_arp": command = "show arp"
        case "get_state":
            switch resource {
            case "arp": command = "show arp"
            case "routes": command = "show ip route"
            case "interfaces": command = "show interfaces"
            case "lldp": command = "show lldp neighbors"
            case "mac_table": command = "show mac address-table"
            case "bgp": command = "show ip bgp summary"
            case "ospf": command = "show ip ospf neighbor"
            case "cpu": command = CPUUsagePolicy.command(for: connection.deviceType)
            default: return NetworkOperationOutput(success: false, stdout: "", stderr: "未対応の状態リソースです。")
            }
        default:
            return NetworkOperationOutput(success: false, stdout: "", stderr: "この読み取りツールはSwift transportで許可されていません。")
        }
        let credentials = credentialPersistence.load(for: connection.id)
        let type = connection.deviceType.lowercased()
        let deviceType: String = {
            if type.contains("juniper") { return "juniper_junos" }
            if type.contains("nx-os") || type.contains("nxos") { return "cisco_nxos" }
            if type.contains("arista") { return "arista_eos" }
            if type.contains("yamaha") { return "yamaha" }
            if type.contains("furukawa") || type.contains("fitel") { return "furukawa_fitelnet" }
            if type.contains("cisco") { return "cisco_ios" }
            return type.replacingOccurrences(of: " ", with: "_")
        }()
        let request = NetworkRunnerRequest(
            action: "show",
            host: connection.host,
            username: connection.username,
            password: credentials.password ?? "",
            secret: credentials.enablePassword ?? "",
            deviceType: deviceType,
            port: connection.port,
            commands: [command]
        )
        let result = runNetworkWrapper(request)
        if tool == "get_state", resource == "cpu" {
            guard result.success else { return result }
            guard let usage = CPUUsagePolicy.parse(result.stdout) else {
                return NetworkOperationOutput(success: false, stdout: "", stderr: "CPU使用率を機器出力から数値として取得できませんでした。")
            }
            return NetworkOperationOutput(success: true, stdout: "{\"usage\":\(usage)}", stderr: "")
        }
        return result
    }

    private nonisolated static func portableAsset(_ relativePath: String) -> URL? {
        let cwd = URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
        let roots = [
            ProcessInfo.processInfo.environment["MIKOMAI_ASSETS_DIR"].map { URL(fileURLWithPath: $0) },
            Bundle.main.resourceURL,
            cwd.appendingPathComponent("mikomai-core/assets"),
            cwd.appendingPathComponent("../mikomai-core/assets").standardizedFileURL
        ].compactMap { $0 }
        return roots.map { $0.appendingPathComponent(relativePath) }.first { FileManager.default.fileExists(atPath: $0.path) }
    }

    private nonisolated static func portablePython() -> URL? {
        let candidates = [
            ProcessInfo.processInfo.environment["MIKOMAI_PYTHON"].map { URL(fileURLWithPath: $0) },
            URL(fileURLWithPath: FileManager.default.currentDirectoryPath).appendingPathComponent("venv/bin/python"),
            URL(fileURLWithPath: "/usr/bin/python3")
        ].compactMap { $0 }
        return candidates.first { FileManager.default.isExecutableFile(atPath: $0.path) }
    }

    private nonisolated static func runJSONPython(script: URL, python: URL, payload: [String: Any]) -> NetworkOperationOutput {
        do {
            let process = Process(); process.executableURL = python; process.arguments = [script.path]
            let input = Pipe(); let output = Pipe(); let error = Pipe()
            process.standardInput = input; process.standardOutput = output; process.standardError = error
            try process.run()
            let data = try JSONSerialization.data(withJSONObject: payload)
            input.fileHandleForWriting.write(data); input.fileHandleForWriting.closeFile()
            let stdout = String(decoding: output.fileHandleForReading.readDataToEndOfFile(), as: UTF8.self)
            let stderr = String(decoding: error.fileHandleForReading.readDataToEndOfFile(), as: UTF8.self)
            process.waitUntilExit()
            guard process.terminationStatus == 0,
                  let decoded = try JSONSerialization.jsonObject(with: Data(stdout.utf8)) as? [String: Any] else {
                return NetworkOperationOutput(success: false, stdout: stdout, stderr: stderr.isEmpty ? "Config helperの出力を解析できません。" : stderr)
            }
            let success = decoded["success"] as? Bool ?? false
            let pretty = (try? JSONSerialization.data(withJSONObject: decoded, options: [.prettyPrinted, .sortedKeys])).map { String(decoding: $0, as: UTF8.self) } ?? stdout
            return NetworkOperationOutput(success: success, stdout: success ? pretty : "", stderr: success ? "" : (decoded["error"] as? String ?? pretty))
        } catch { return NetworkOperationOutput(success: false, stdout: "", stderr: "Config helperを実行できません: \(error.localizedDescription)") }
    }

    private nonisolated static func runAgentUtility(_ executable: String, _ arguments: [String]) -> NetworkOperationOutput {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: executable)
        process.arguments = arguments
        let out = Pipe(); let err = Pipe()
        process.standardOutput = out; process.standardError = err
        do {
            try process.run(); process.waitUntilExit()
            let stdout = String(decoding: out.fileHandleForReading.readDataToEndOfFile(), as: UTF8.self)
            let stderr = String(decoding: err.fileHandleForReading.readDataToEndOfFile(), as: UTF8.self)
            return NetworkOperationOutput(success: process.terminationStatus == 0, stdout: stdout, stderr: stderr)
        } catch { return NetworkOperationOutput(success: false, stdout: "", stderr: error.localizedDescription) }
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
            saveSettings()
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

    func openSettingsDirectory() {
        let dir = settingsFileURL.deletingLastPathComponent()
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

    func importLegacyDevices(fromJSON data: Data) throws -> LegacyConnectionImportResult {
        let result = try LegacyConnectionImporter.importJSON(data, existing: connections)
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
        connections: [SavedConnection],
        credentialPersistence: ConnectionCredentialPersistence,
        onOperationPlan: @escaping (Data) -> Void,
        onChunk: @escaping (String, Bool) -> Void
    ) -> String {
        let box = ChatCallbackBox(stream: StreamBox(onChunk: onChunk), connections: connections, credentialPersistence: credentialPersistence, onOperationPlan: onOperationPlan)
        let context = Unmanaged.passUnretained(box).toOpaque()
        let publicDevices = connections.map { connection in
            ["id": connection.id.uuidString, "hostname": connection.name, "ip": connection.host, "deviceType": connection.deviceType]
        }
        let devicesJSON = (try? JSONSerialization.data(withJSONObject: publicDevices)).map { String(decoding: $0, as: UTF8.self) } ?? "[]"
        let response = prompt.withCString { message in
            devicesJSON.withCString { devices in
                let routeResult = mikomai_dispatch_mode(message, devices)
                let mode = routeResult.message.map { String(cString: $0) } ?? "worker"
                mikomai_result_free(routeResult)
                if mode == "agent" {
                    return history.withCString { historyText in
                        documents.withCString { documentsPath in
                            knowledge.withCString { knowledgePath in
                                attachments.withCString { attachmentText in
                                    mikomai_agent_chat_streaming(message, historyText, documentsPath, knowledgePath, attachmentText, devices, streamBridge, agentToolBridge, agentPlanBridge, context)
                                }
                            }
                        }
                    }
                }
                return history.withCString { historyText in
                    documents.withCString { documentsPath in
                        knowledge.withCString { knowledgePath in
                            attachments.withCString { attachmentText in
                                mikomai_assistant_chat_streaming(message, historyText, documentsPath, knowledgePath, attachmentText, streamBridge, context)
                            }
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
        consumeRust(call())
    }

    private nonisolated static func consumeRust(_ response: MikomaiResult) -> String {
        defer { mikomai_result_free(response) }
        guard let message = response.message else { return "応答がありませんでした。" }
        let text = String(cString: message)
        return response.status == 0 ? text : "エラー: \(text)"
    }

    fileprivate nonisolated static func executeApprovedAgentOperation(planID: String, planHash: String, password: String?) -> NetworkOperationOutput {
        let credentials: String
        do { credentials = String(decoding: try JSONSerialization.data(withJSONObject: ["password": password ?? ""]), as: UTF8.self) }
        catch { return NetworkOperationOutput(success: false, stdout: "", stderr: error.localizedDescription) }
        let response = planID.withCString { id in
            planHash.withCString { hash in
                credentials.withCString { secretJSON in
                    mikomai_operation_execute_approved(id, hash, secretJSON)
                }
            }
        }
        defer { mikomai_result_free(response) }
        guard response.status == 0, let message = response.message else {
            return NetworkOperationOutput(success: false, stdout: "", stderr: response.message.map { String(cString: $0) } ?? "承認済み操作が失敗しました。")
        }
        let text = String(cString: message)
        if let data = text.data(using: .utf8), let payload = try? JSONSerialization.jsonObject(with: data) as? [String: Any] {
            let output = payload["output"] as? String ?? text
            return NetworkOperationOutput(success: true, stdout: output, stderr: "")
        }
        return NetworkOperationOutput(success: true, stdout: text, stderr: "")
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

private struct ChatBottomPreferenceKey: PreferenceKey {
    static let defaultValue: CGFloat = 0
    static func reduce(value: inout CGFloat, nextValue: () -> CGFloat) { value = nextValue() }
}

private struct PaneResizeCursor: NSViewRepresentable {
    final class CursorView: NSView {
        override func resetCursorRects() {
            addCursorRect(bounds, cursor: .resizeLeftRight)
        }
    }

    func makeNSView(context: Context) -> CursorView { CursorView() }
    func updateNSView(_ view: CursorView, context: Context) {
        view.window?.invalidateCursorRects(for: view)
    }
}

private struct ChatTopPreferenceKey: PreferenceKey {
    static let defaultValue: CGFloat = 0
    static func reduce(value: inout CGFloat, nextValue: () -> CGFloat) { value = nextValue() }
}

private struct DesktopWindow: View {
    @ObservedObject var model: DesktopModel
    @State private var mentionPresentation = ChatMentionPresentation()
    @State private var isChatInputFocused = false
    private var mentionContext: ChatMentionContext? { mentionPresentation.context }
    private var showsHostSuggestions: Bool { mentionPresentation.isVisible(candidateCount: hostSuggestions.count) }
    @State private var hostSuggestionIndex = 0
    @State private var mentionCompletion: ChatMentionCompletion?


    @AppStorage("mikomai.desktop.mac.historyWidth") private var historyWidth = 248.0
    @State private var historyDragStart: CGFloat?
    @State private var isHistoryOpen = true
    @AppStorage("mikomai.desktop.mac.rightPaneWidth") private var rightPaneWidth = 330.0
    @State private var rightPaneDragStart: CGFloat?
    @State private var isRightPaneOpen = false
    @State private var rightPaneTab = "diff"
    @State private var isAtChatBottom = true
    @State private var chatScrollFollow = ChatScrollFollowState()
    @State private var selectedConnectionID: UUID?
    @State private var operationAlert = ""
    @State private var isOperationRunning = false
    @State private var operationRationale = "選択した変更案を適用する"

    private func historyMaximumWidth(containerWidth: CGFloat) -> CGFloat {
        CGFloat(PaneResizePolicy.maximumWidth(
            containerWidth: Double(containerWidth),
            reservedWidth: 50 + 440 + (isRightPaneOpen ? rightPaneWidth + 8 : 0) + 8,
            lowerBound: 180,
            upperBound: 420
        ))
    }

    private func rightPaneMaximumWidth(containerWidth: CGFloat) -> CGFloat {
        let visibleHistoryWidth = isHistoryOpen ? min(CGFloat(historyWidth), historyMaximumWidth(containerWidth: containerWidth)) + 8 : 0
        return CGFloat(PaneResizePolicy.maximumWidth(
            containerWidth: Double(containerWidth),
            reservedWidth: 50 + 440 + Double(visibleHistoryWidth) + 8,
            lowerBound: 180,
            upperBound: 600
        ))
    }

    private func paneResizeDivider(isHistory: Bool, currentWidth: CGFloat, maximumWidth: CGFloat) -> some View {
        Rectangle()
            .fill(Color(nsColor: .separatorColor).opacity(0.65))
            .frame(width: 1)
            .frame(width: 8)
            .background(PaneResizeCursor())
            .contentShape(Rectangle())
            .gesture(DragGesture(minimumDistance: 1, coordinateSpace: .global)
                .onChanged { value in
                    if isHistory {
                        if historyDragStart == nil { historyDragStart = currentWidth }
                        historyWidth = PaneResizePolicy.clampedWidth(
                            Double((historyDragStart ?? currentWidth) + value.translation.width),
                            maximumWidth: Double(maximumWidth)
                        )
                    } else {
                        if rightPaneDragStart == nil { rightPaneDragStart = currentWidth }
                        rightPaneWidth = PaneResizePolicy.clampedWidth(
                            Double((rightPaneDragStart ?? currentWidth) - value.translation.width),
                            maximumWidth: Double(maximumWidth)
                        )
                    }
                }
                .onEnded { value in
                    if isHistory {
                        if PaneResizePolicy.shouldClose(
                            startWidth: Double(historyDragStart ?? currentWidth),
                            translation: Double(value.translation.width),
                            isHistoryPane: true
                        ) { isHistoryOpen = false }
                        historyDragStart = nil
                    } else {
                        if PaneResizePolicy.shouldClose(
                            startWidth: Double(rightPaneDragStart ?? currentWidth),
                            translation: Double(value.translation.width),
                            isHistoryPane: false
                        ) { isRightPaneOpen = false }
                        rightPaneDragStart = nil
                    }
                })
            .help(isHistory ? "ドラッグして会話履歴の幅を調整・180pt未満で閉じる" : "ドラッグして右ペインの幅を調整・180pt未満で閉じる")
            .accessibilityLabel(isHistory ? "会話履歴の幅を調整" : "右ペインの幅を調整")
    }

    private var hostSuggestions: [HostSuggestion] {
        guard let context = mentionContext else { return [] }
        let hosts = model.availableCompletionHosts
        return HostSuggestionPolicy.find(
            query: context.query,
            availableHosts: hosts,
            recentIPs: model.settings.recentIps,
            labels: HostSuggestionLabels(localhost: "このコンピュータ", pastIps: "過去に投入したIPアドレス")
        )
    }

    private func selectHostSuggestion(_ suggestion: HostSuggestion) {
        mentionCompletion = ChatMentionCompletion(hostname: suggestion.hostname)
        mentionPresentation.dismiss()
        isChatInputFocused = true
    }

    private func handleSuggestionKey(_ key: ChatSuggestionKey) -> Bool {
        guard showsHostSuggestions else { return false }
        switch key {
        case .next: hostSuggestionIndex = (hostSuggestionIndex + 1) % hostSuggestions.count
        case .previous: hostSuggestionIndex = (hostSuggestionIndex + hostSuggestions.count - 1) % hostSuggestions.count
        case .accept: selectHostSuggestion(hostSuggestions[min(hostSuggestionIndex, hostSuggestions.count - 1)])
        case .dismiss: mentionPresentation.dismiss()
        }
        return true
    }

    var body: some View {
        GeometryReader { geometry in
        HStack(spacing: 0) {
            activityBar
            if model.workspace == .chat && isHistoryOpen {
                historySidebar
                    .frame(width: min(CGFloat(historyWidth), historyMaximumWidth(containerWidth: geometry.size.width)))
                paneResizeDivider(isHistory: true,
                    currentWidth: min(CGFloat(historyWidth), historyMaximumWidth(containerWidth: geometry.size.width)),
                    maximumWidth: historyMaximumWidth(containerWidth: geometry.size.width))
            }
            HStack(spacing: 0) {
                VStack(spacing: 0) {
                    header
                    Group {
                        switch model.workspace {
                        case .chat: chatWorkspace
                        case .connections: ConnectionsWorkspace(model: model)
                        case .tools: NetworkToolsWorkspace(model: model)
                        case .monitoring: MonitoringWorkspace(model: model)
                        case .settings: SettingsWorkspace(model: model)
                        }
                    }
                    statusBar
                }
                .background(Color(nsColor: .windowBackgroundColor))
                if model.workspace == .chat && isRightPaneOpen {
                    paneResizeDivider(isHistory: false,
                        currentWidth: min(CGFloat(rightPaneWidth), rightPaneMaximumWidth(containerWidth: geometry.size.width)),
                        maximumWidth: rightPaneMaximumWidth(containerWidth: geometry.size.width))
                    rightSidePane
                        .frame(width: min(CGFloat(rightPaneWidth), rightPaneMaximumWidth(containerWidth: geometry.size.width)))
                        .transition(.move(edge: .trailing))
                }
            }
        }
        .background(Color(nsColor: .underPageBackgroundColor))
        }
        .sheet(item: $model.editingConnection) { connection in
            ConnectionEditor(connection: connection) { saved, pwd, enPwd in
                model.saveConnection(saved, password: pwd, enablePassword: enPwd)
            }
        }
        .onChange(of: model.operationPlan?.id) { _ in
            guard let plan = model.operationPlan,
                  let id = UUID(uuidString: plan.args.deviceSnapshot.id) else { return }
            selectedConnectionID = id
            model.workspace = .chat
            rightPaneTab = "diff"
            isRightPaneOpen = true
        }
        .onAppear { model.startWatchService() }
        .onReceive(NotificationCenter.default.publisher(for: NSApplication.willTerminateNotification)) { _ in model.stopWatchService() }
        .alert(item: $model.watchAlert) { alert in
            Alert(title: Text("ネットワーク監視"), message: Text(alert.message), dismissButton: .default(Text("閉じる")))
        }
    }

    private var activityBar: some View {
        VStack(spacing: 8) {
            ForEach(Workspace.allCases.filter { $0 != .settings }) { item in
                activityButton(item)
            }
            Spacer()
            activityButton(.settings)
        }
        .padding(.vertical, 12).frame(width: 50)
        .background(Color(nsColor: .controlBackgroundColor))
        .overlay(alignment: .trailing) { Divider() }
    }

    private func activityButton(_ item: Workspace) -> some View {
        Button {
            model.workspace = item
            if item == .chat { isHistoryOpen = true }
        } label: {
            Image(systemName: item.icon).font(.system(size: 16, weight: .medium))
                .foregroundStyle(model.workspace == item ? .primary : .secondary)
                .frame(width: 34, height: 34)
                .background(model.workspace == item ? Color(nsColor: .selectedContentBackgroundColor).opacity(0.16) : .clear, in: RoundedRectangle(cornerRadius: 6))
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain).help(item.rawValue)
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
                Button { withAnimation(.easeInOut(duration: 0.18)) { isRightPaneOpen.toggle() } } label: {
                    Image(systemName: "sidebar.right")
                        .font(.system(size: 13, weight: .medium))
                        .frame(width: 28, height: 26)
                        .background(isRightPaneOpen ? Color.accentColor.opacity(0.15) : .clear, in: RoundedRectangle(cornerRadius: 5))
                }
                .buttonStyle(.plain).help(isRightPaneOpen ? "右ペインを閉じる" : "差分とログを表示")
            }
        }
        .padding(.horizontal, 20).padding(.vertical, 12)
        .background(Color(nsColor: .windowBackgroundColor))
        .overlay(alignment: .bottom) { Divider() }
    }

    private var rightSidePane: some View {
        VStack(spacing: 0) {
            HStack {
                Text("作業パネル").font(.system(size: 13, weight: .semibold))
                Spacer()
                Button { isRightPaneOpen = false } label: { Image(systemName: "xmark") }
                    .buttonStyle(.plain).foregroundStyle(.secondary).help("右ペインを閉じる")
            }.padding(.horizontal, 14).padding(.vertical, 12)
            Divider()
            HStack(spacing: 12) {
                WorkspaceTabButton(title: "Diff", icon: "arrow.left.arrow.right", isSelected: rightPaneTab == "diff") {
                    rightPaneTab = "diff"
                }
                WorkspaceTabButton(title: "ログ", icon: "text.alignleft", isSelected: rightPaneTab == "logs") {
                    rightPaneTab = "logs"
                }
                Spacer()
            }
            .padding(.horizontal, 16).padding(.vertical, 8)
            .background(Color(nsColor: .controlBackgroundColor))
            Divider()
            if rightPaneTab == "diff" {
                operationDiffPane
            } else {
                VStack(alignment: .leading, spacing: 10) {
                    Label("投入ログ", systemImage: "text.alignleft").font(.system(size: 12, weight: .semibold))
                    if model.operationLogs.isEmpty {
                        Text("変更案の確認と投入を行うと、各手順の結果がここに表示されます。")
                            .font(.system(size: 11)).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
                    } else {
                        ScrollView {
                            VStack(alignment: .leading, spacing: 5) {
                                ForEach(Array(model.operationLogs.enumerated()), id: \.offset) { _, line in
                                    Text(line).font(.system(size: 10, design: .monospaced)).textSelection(.enabled)
                                        .frame(maxWidth: .infinity, alignment: .leading)
                                }
                            }
                        }
                    }
                    Spacer()
                }.padding(14).frame(maxWidth: .infinity, alignment: .leading)
            }
        }
        .background(Color(nsColor: .controlBackgroundColor).opacity(0.72))
        .overlay(alignment: .leading) { Divider() }
    }

    private var operationDiffPane: some View {
        VStack(alignment: .leading, spacing: 10) {
            Label("変更計画", systemImage: "doc.text.magnifyingglass")
                .font(.system(size: 12, weight: .semibold))
            if model.operationProposal.isEmpty {
                Text("回答の設定コマンドを右クリックし、「変更計画として確認」を選ぶと、ここで現状との差分を確認できます。")
                    .font(.system(size: 11)).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
                Spacer()
            } else {
                Picker("対象機器", selection: $selectedConnectionID) {
                    Text("機器を選択").tag(Optional<UUID>.none)
                    ForEach(model.connections.filter { ($0.connectionType ?? "SSH").lowercased() == "ssh" }) { connection in
                        Text("\(connection.name) (\(connection.host))").tag(Optional(connection.id))
                    }
                }
                .disabled(model.operationPlan != nil || isOperationRunning)
                TextEditor(text: $model.operationProposal)
                    .font(.system(size: 10, design: .monospaced)).frame(minHeight: 95, maxHeight: 170)
                    .disabled(model.operationPlan != nil || isOperationRunning)
                DisclosureGroup("取得した現状のConfig") {
                    ScrollView {
                        Text(model.operationBeforeConfig.isEmpty ? "まだ取得していません。" : model.operationBeforeConfig)
                            .font(.system(size: 9, design: .monospaced)).textSelection(.enabled)
                            .frame(maxWidth: .infinity, alignment: .leading)
                    }.frame(maxHeight: 110)
                }
                TextField("変更の理由", text: $operationRationale)
                    .textFieldStyle(.roundedBorder).font(.system(size: 11))
                    .disabled(model.operationPlan != nil || isOperationRunning)
                if !model.operationBeforeConfig.isEmpty {
                    Text(model.operationAfterConfig.isEmpty ? "提案コマンド" : "投入後の実機差分").font(.system(size: 11, weight: .semibold))
                    ScrollView {
                        VStack(alignment: .leading, spacing: 2) {
                            ForEach(Array(operationPreviewLines.enumerated()), id: \.offset) { _, item in
                                Text(item).font(.system(size: 10, design: .monospaced))
                                    .foregroundStyle(item.hasPrefix("+") ? Color.green : Color.secondary)
                                    .frame(maxWidth: .infinity, alignment: .leading)
                            }
                        }
                    }.frame(maxHeight: 180)
                }
                if let plan = model.operationPlan {
                    Text("状態: \(operationStatusLabel(plan.status))")
                        .font(.system(size: 11, weight: .medium)).foregroundStyle(.secondary)
                    DisclosureGroup("計画の照合情報") {
                        Text("ID: \(plan.id)\nSHA-256: \(plan.planHash)")
                            .font(.system(size: 9, design: .monospaced)).textSelection(.enabled)
                            .foregroundStyle(.secondary).lineLimit(4)
                    }
                }
                if !operationAlert.isEmpty {
                    Text(operationAlert).font(.system(size: 10)).foregroundStyle(.red).fixedSize(horizontal: false, vertical: true)
                }
                Spacer(minLength: 0)
                if isOperationRunning {
                    HStack(spacing: 7) { ProgressView().controlSize(.small); Text(model.operationPhase).font(.system(size: 11)) }
                } else if model.operationPlan == nil {
                    Button("現状を取得して差分を確認") { Task { await prepareOperationPlan() } }
                        .buttonStyle(.borderedProminent).disabled(selectedConnectionID == nil || model.connections.isEmpty)
                } else if model.operationPlan?.status == "pending" {
                    Button("確認して承認・投入") { Task { await approveAndExecutePlan() } }
                        .buttonStyle(.borderedProminent)
                }
            }
        }
        .padding(14).frame(maxWidth: .infinity, alignment: .leading)
    }

    private var operationPreviewLines: [String] {
        if !model.operationAfterConfig.isEmpty {
            return model.operationDiffLines
        }
        return model.operationProposal.split(whereSeparator: \.isNewline).map { "+ \($0)" }
    }

    nonisolated private static func lineDiff(old: String, new: String) -> [String] {
        let oldLines = old.components(separatedBy: .newlines)
        let newLines = new.components(separatedBy: .newlines)
        let changes = newLines.difference(from: oldLines)
        let edits: [(Int, Int, String)] = changes.compactMap { change in
            switch change {
            case let .remove(offset, element, _): (offset, 0, "- \(element)")
            case let .insert(offset, element, _): (offset, 1, "+ \(element)")
            }
        }
        return edits.sorted { ($0.0, $0.1) < ($1.0, $1.1) }.map(\.2)
    }

    private func prepareOperationPlan() async {
        guard !isOperationRunning, let id = selectedConnectionID,
              let connection = model.connections.first(where: { $0.id == id }) else { return }
        guard let request = model.networkRequest(action: "show", connection: connection, commands: [showConfigCommand(for: connection)]) else {
            operationAlert = "現在、Console 接続の変更計画には対応していません。SSH 接続の機器を選んでください。"
            return
        }
        isOperationRunning = true
        operationAlert = ""
        model.operationLogs.append("[STATUS] 1/4 現状のConfigを取得中")
        model.operationPhase = "現状のConfigを取得中…"
        let output = await Task.detached { DesktopModel.runNetworkWrapper(request) }.value
        if !output.stderr.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            model.operationLogs.append(contentsOf: output.stderr.split(whereSeparator: \.isNewline).map(String.init))
        }
        guard output.success, !output.stdout.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
              !networkOutputHasError(output.stdout) else {
            model.operationLogs.append("[ERROR] 現状Config取得に失敗しました。変更計画は作成していません。")
            operationAlert = "現状のConfigを取得できませんでした。機器情報と接続を確認してください。"
            model.operationPhase = "現状取得失敗"
            isOperationRunning = false
            return
        }
        model.operationBeforeConfig = output.stdout
        if let error = model.createOperationPlan(target: connection, proposal: model.operationProposal, rationale: operationRationale) {
            operationAlert = error
            model.operationPhase = "計画作成失敗"
            isOperationRunning = false
            return
        }
        model.operationLogs.append("[STATUS] 現状取得後、機器・コマンドに固定した変更計画を作成しました")
        isOperationRunning = false
    }

    private func approveAndExecutePlan() async {
        guard !isOperationRunning, let plan = model.operationPlan,
              plan.status == "pending", let (connection, credentials) = model.resolveOperationTarget(for: plan) else {
            operationAlert = "計画作成後に対象機器の情報が変わりました。変更計画を作り直してください。"
            return
        }
        isOperationRunning = true
        operationAlert = ""
        guard model.approveOperationPlan() == nil, model.beginOperationPlan() == nil else {
            operationAlert = "変更計画を承認できませんでした。"
            model.operationLogs.append("[ERROR] ハッシュ照合による承認に失敗しました")
            model.operationPhase = "承認失敗"
            isOperationRunning = false
            return
        }
        if plan.toolId != "network_config" {
            model.operationPhase = "承認済み操作を実行中…"
            model.operationLogs.append("[STATUS] 承認済み操作を実行中")
            rightPaneTab = "logs"
            let output = await Task.detached {
                DesktopModel.executeApprovedAgentOperation(planID: plan.id, planHash: plan.planHash, password: credentials.password)
            }.value
            model.operationLogs.append(contentsOf: output.stdout.split(whereSeparator: \.isNewline).map(String.init))
            if !output.stderr.isEmpty { model.operationLogs.append(contentsOf: output.stderr.split(whereSeparator: \.isNewline).map(String.init)) }
            model.finishOperationPlan(succeeded: output.success)
            model.operationPhase = output.success ? "承認済み操作が完了しました" : "承認済み操作が失敗しました"
            if !output.success { operationAlert = "操作に失敗しました。ログを確認してください。" }
            isOperationRunning = false
            return
        }
        let planCommands = plan.args.commands ?? []
        guard !planCommands.isEmpty else {
            model.finishOperationPlan(succeeded: false)
            operationAlert = "この操作はSwift側の承認済み実行経路がまだ接続されていません。"
            model.operationPhase = "実行経路未接続"
            isOperationRunning = false
            return
        }
        let target = plan.args.deviceSnapshot
        let approvedRequest = NetworkRunnerRequest(
            action: "dry_run", host: target.host, username: target.username,
            password: credentials.password ?? "", secret: credentials.enablePassword ?? "",
            deviceType: runnerDeviceType(target.deviceType), port: target.port, commands: planCommands
        )
        model.operationPhase = "2/4 dry-run 検証中…"
        model.operationLogs.append("[STATUS] 2/4 dry-run 検証中")
        rightPaneTab = "logs"
        let configRequest = NetworkRunnerRequest(
            action: "config", host: target.host, username: target.username,
            password: credentials.password ?? "", secret: credentials.enablePassword ?? "",
            deviceType: runnerDeviceType(target.deviceType), port: target.port, commands: planCommands
        )
        let workflow = await OperationWorkflow.execute(
            dryRun: {
                let output = await Task.detached { DesktopModel.runNetworkWrapper(approvedRequest) }.value
                return OperationCommandOutput(processSucceeded: output.success, stdout: output.stdout, stderr: output.stderr)
            },
            configure: {
                await MainActor.run {
                    model.operationPhase = "3/4 Config 投入中…"
                    model.operationLogs.append("[STATUS] 3/4 Configを投入中")
                }
                let output = await Task.detached { DesktopModel.runNetworkWrapper(configRequest) }.value
                return OperationCommandOutput(processSucceeded: output.success, stdout: output.stdout, stderr: output.stderr)
            }
        )
        let dryRun = workflow.dryRun
        model.operationLogs.append(contentsOf: dryRun.stderr.split(whereSeparator: \.isNewline).map(String.init))
        guard workflow.dryRunPassed, let deployed = workflow.configuration else {
            model.operationLogs.append("[ERROR] dry-runに失敗したためConfig投入を中止しました")
            model.operationPhase = "dry-run失敗"
            model.finishOperationPlan(succeeded: false)
            operationAlert = "dry-runでエラーが見つかったため、機器への投入を中止しました。"
            isOperationRunning = false
            return
        }
        model.operationLogs.append(contentsOf: deployed.stderr.split(whereSeparator: \.isNewline).map(String.init))
        guard deployed.processSucceeded else {
            model.operationLogs.append("[ERROR] Config投入に失敗しました")
            model.operationPhase = "投入失敗"
            model.finishOperationPlan(succeeded: false)
            operationAlert = "Configを投入できませんでした。ログを確認してください。"
            isOperationRunning = false
            return
        }
        model.operationPhase = "4/4 投入後のConfigを検証中…"
        model.operationLogs.append("[STATUS] 4/4 投入後のConfigを取得して差分を検証中")
        let verifyRequest = NetworkRunnerRequest(
            action: "show", host: target.host, username: target.username,
            password: credentials.password ?? "", secret: credentials.enablePassword ?? "",
            deviceType: runnerDeviceType(target.deviceType), port: target.port,
            commands: [showConfigCommand(for: connection)]
        )
        let verified = await Task.detached { DesktopModel.runNetworkWrapper(verifyRequest) }.value
        model.operationLogs.append(contentsOf: verified.stderr.split(whereSeparator: \.isNewline).map(String.init))
        guard verified.success, !verified.stdout.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
              !networkOutputHasError(verified.stdout) else {
            model.operationLogs.append("[ERROR] 投入後Configの取得に失敗しました")
            model.operationPhase = "検証失敗"
            model.finishOperationPlan(succeeded: false)
            operationAlert = "Configは投入されましたが、投入後の状態を確認できませんでした。"
            isOperationRunning = false
            return
        }
        let before = model.operationBeforeConfig
        let after = verified.stdout
        let diff = await Task.detached { Self.lineDiff(old: before, new: after) }.value
        model.operationAfterConfig = after
        model.operationDiffLines = diff
        model.finishOperationPlan(succeeded: true)
        model.operationPhase = "投入後Configを取得しました。差分を確認してください"
        model.operationLogs.append("[STATUS] Config投入が成功し、投入後Configを取得しました。差分を確認してください")
        rightPaneTab = "diff"
        isOperationRunning = false
    }

    private func networkOutputHasError(_ output: String) -> Bool {
        let lower = output.lowercased()
        return ["% invalid input", "% incomplete command", "% ambiguous command", "syntax error", "netmiko error:", "error: device"].contains { lower.contains($0) }
    }

    private func operationStatusLabel(_ status: String) -> String {
        switch status {
        case "pending": "承認待ち"
        case "approved": "承認済み"
        case "executing": "実行中"
        case "executed": "完了"
        case "failed": "失敗"
        case "rejected": "却下"
        default: status
        }
    }

    private func showConfigCommand(for connection: SavedConnection) -> String {
        let device = runnerDeviceType(connection.deviceType)
        if device == "juniper_junos" { return "show configuration" }
        if device == "yamaha" { return "show config" }
        return "show running-config"
    }

    private func runnerDeviceType(_ value: String) -> String {
        let lower = value.lowercased()
        if lower.contains("juniper") { return "juniper_junos" }
        if lower.contains("nx-os") || lower.contains("nxos") { return "cisco_nxos" }
        if lower.contains("arista") { return "arista_eos" }
        if lower.contains("yamaha") { return "yamaha" }
        if lower.contains("furukawa") || lower.contains("fitel") { return "furukawa_fitelnet" }
        if lower.contains("cisco") { return "cisco_ios" }
        return lower.replacingOccurrences(of: " ", with: "_")
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
            GeometryReader { viewport in
                ScrollViewReader { proxy in
                    ScrollView {
                    LazyVStack(alignment: .leading, spacing: 18) {
                        if model.activeSession?.messages.isEmpty ?? true { emptyState }
                        if let session = model.activeSession {
                            ForEach(session.messages) { message in
                                MessageRow(message: message, onSelectConfig: { config in
                                    guard !isOperationRunning else { return }
                                    model.operationProposal = config
                                    model.operationPlan = nil
                                    model.operationBeforeConfig = ""
                                    model.operationAfterConfig = ""
                                    model.operationDiffLines = []
                                    model.operationLogs = []
                                    model.operationPhase = "変更案を確認中"
                                    operationRationale = "選択した変更案を適用する"
                                    isRightPaneOpen = true
                                    rightPaneTab = "diff"
                                }).id(message.id)
                            }
                        }
                        if model.isWorking {
                            HStack(spacing: 8) {
                                ProgressView().controlSize(.small)
                                Text(model.isCancelling ? "生成を停止しています…" : "資料を検索して回答を生成しています…")
                                    .font(.system(size: 12)).foregroundStyle(.secondary)
                            }
                            .frame(maxWidth: .infinity, alignment: .leading)
                        }
                        GeometryReader { bottomProxy in
                            Color.clear.preference(key: ChatBottomPreferenceKey.self,
                                                   value: bottomProxy.frame(in: .named("chatScroll")).maxY)
                        }
                        .frame(height: 1)
                        .id("chatBottom")
                    }
                    .background(GeometryReader { topProxy in
                        Color.clear.preference(key: ChatTopPreferenceKey.self,
                            value: topProxy.frame(in: .named("chatScroll")).minY)
                    })
                    .frame(maxWidth: 760).frame(maxWidth: .infinity).padding(.horizontal, 24).padding(.vertical, 24)
                    }
                    .coordinateSpace(name: "chatScroll")
                    .onPreferenceChange(ChatTopPreferenceKey.self) { topY in
                        chatScrollFollow.observe(contentTop: Double(topY), isAtBottom: isAtChatBottom)
                    }
                    .onPreferenceChange(ChatBottomPreferenceKey.self) { bottomY in
                        isAtChatBottom = bottomY <= viewport.size.height + 32
                        chatScrollFollow.updateViewport(isAtBottom: isAtChatBottom)
                    }
                    .onChange(of: model.activeSession?.messages.last?.text ?? "") { _ in
                        if chatScrollFollow.followsOutput { proxy.scrollTo("chatBottom", anchor: .bottom) }
                    }
                    .onChange(of: model.activeSession?.messages.count ?? 0) { _ in
                        if chatScrollFollow.followsOutput { proxy.scrollTo("chatBottom", anchor: .bottom) }
                    }
                    .onChange(of: model.activeSessionID) { _ in
                        isAtChatBottom = true
                        chatScrollFollow.resetForSessionChange()
                        proxy.scrollTo("chatBottom", anchor: .bottom)
                    }
                    .overlay(alignment: .bottom) {
                        if !isAtChatBottom {
                            Button {
                                chatScrollFollow.resume()
                                proxy.scrollTo("chatBottom", anchor: .bottom)
                            } label: {
                                Label("一番下に移動", systemImage: "arrow.down")
                                    .font(.system(size: 12, weight: .medium))
                                    .padding(.horizontal, 14).padding(.vertical, 8)
                                    .background(Color(nsColor: .controlBackgroundColor), in: RoundedRectangle(cornerRadius: 8))
                                    .overlay(RoundedRectangle(cornerRadius: 8).stroke(Color(nsColor: .separatorColor), lineWidth: 0.7))
                            }
                            .buttonStyle(.plain).padding(.bottom, 8)
                        }
                    }
                }
            }
            composer
        }
    }

    private var emptyState: some View {
        VStack(spacing: 24) {
            if let iconURL = Bundle.module.url(forResource: "AppIcon", withExtension: "icns"),
               let icon = NSImage(contentsOf: iconURL) {
                Image(nsImage: icon)
                    .resizable()
                    .scaledToFit()
                    .frame(width: 72, height: 72)
                    .accessibilityHidden(true)
            } else {
                Image(systemName: "network")
                    .font(.system(size: 48))
                    .foregroundStyle(Color.accentColor)
                    .accessibilityHidden(true)
            }
            Text("インフラについて何を行いますか？")
                .font(.system(size: 21, weight: .semibold))
                .multilineTextAlignment(.center)
            LazyVGrid(columns: [GridItem(.flexible()), GridItem(.flexible())], spacing: 12) {
                suggestion("VLANの設定方法を調べる", icon: "network",
                    prompt: "F220のVLAN設定方法を、設定例と確認コマンドを含めて教えてください。")
                suggestion("MACアドレスを確認する", icon: "desktopcomputer",
                    prompt: "CiscoスイッチでMACアドレステーブルを確認するコマンドと、結果の読み方を教えてください。")
                suggestion("通信トラブルを調査する", icon: "antenna.radiowaves.left.and.right",
                    prompt: "ネットワークの通信トラブルを切り分けるための、PingとTracerouteを使った調査手順を教えてください。")
                suggestion("サブネットを設計する", icon: "square.grid.2x2",
                    prompt: "192.168.10.0/24を4つの同じ大きさのサブネットに分割し、それぞれのネットワークアドレス、利用可能なIP範囲、ブロードキャストアドレスを示してください。")
            }
        }
        .frame(maxWidth: 560)
        .frame(maxWidth: .infinity)
        .padding(.top, 48)
        .padding(.bottom, 24)
    }

    private func suggestion(_ title: String, icon: String, prompt: String) -> some View {
        Button {
            guard !model.isWorking else { return }
            mentionPresentation.dismiss()
            model.draft = prompt
            model.send()
        } label: {
            VStack(alignment: .leading, spacing: 12) {
                Image(systemName: icon)
                    .font(.system(size: 18))
                    .foregroundStyle(Color.accentColor)
                Text(title)
                    .font(.system(size: 13, weight: .medium))
                    .foregroundStyle(.primary)
                    .multilineTextAlignment(.leading)
                    .fixedSize(horizontal: false, vertical: true)
            }
            .frame(maxWidth: .infinity, minHeight: 76, alignment: .leading)
            .padding(16)
            .background(Color(nsColor: .controlBackgroundColor).opacity(0.65), in: RoundedRectangle(cornerRadius: 10))
            .overlay(RoundedRectangle(cornerRadius: 10).stroke(Color(nsColor: .separatorColor).opacity(0.5), lineWidth: 0.7))
            .contentShape(RoundedRectangle(cornerRadius: 10))
        }
        .buttonStyle(.plain)
        .disabled(model.isWorking)
        .help("クリックして実行")
    }

    private var composer: some View {
        VStack(alignment: .leading, spacing: 6) {
            if showsHostSuggestions {
                ScrollViewReader { suggestionProxy in
                ScrollView {
                VStack(alignment: .leading, spacing: 2) {
                    ForEach(Array(hostSuggestions.enumerated()), id: \.element.id) { index, suggestion in
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
                        .background(index == hostSuggestionIndex ? Color.accentColor.opacity(0.14) : .clear,
                                    in: RoundedRectangle(cornerRadius: 4))
                        .id(index)
                        .onHover { hovering in if hovering { hostSuggestionIndex = index } }
                    }
                }
                }
                .frame(height: min(180, CGFloat(hostSuggestions.count) * 29))
                .onChange(of: hostSuggestionIndex) { index in suggestionProxy.scrollTo(index) }
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
                    Image(systemName: "paperclip")
                        .font(.system(size: 13))
                        .foregroundStyle(.secondary)
                        .frame(width: 28, height: 28)
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain).disabled(model.isWorking).help("テキストファイルを添付")

                ChatComposer(text: $model.draft, isFocused: $isChatInputFocused,
                             isEnabled: !model.isWorking, onSubmit: model.send,
                             onEscape: { mentionPresentation.dismiss() },
                             onSuggestionKey: handleSuggestionKey,
                             onMentionContextChanged: { context in
                                 guard mentionContext != context else { return }
                                 hostSuggestionIndex = 0
                                 mentionPresentation.update(context: context)
                                 if context != nil { model.reloadCompletionHosts() }
                             }, completion: mentionCompletion)
                    .padding(.horizontal, 4)
                    .padding(.vertical, 4)

                if model.isWorking {
                    Button(action: model.stop) { Image(systemName: "stop.fill").font(.system(size: 10, weight: .semibold)).frame(width: 28, height: 28) }
                        .buttonStyle(.bordered).controlSize(.small)
                        .disabled(!ChatSubmissionPolicy.canStop(isWorking: model.isWorking, isCancelling: model.isCancelling))
                        .help("生成を停止")
                } else {
                    Button(action: model.send) {
                        Image(systemName: ChatSubmissionPolicy.hasContent(prompt: model.draft, attachmentCount: model.pendingAttachments.count) ? "paperplane.fill" : "arrow.up")
                            .font(.system(size: 12, weight: .semibold))
                            .foregroundStyle(.white).frame(width: 30, height: 30)
                            .background(ChatSubmissionPolicy.hasContent(prompt: model.draft, attachmentCount: model.pendingAttachments.count) ? Color.accentColor : Color.gray.opacity(0.55), in: Circle())
                    }
                        .buttonStyle(.plain)
                        .disabled(!ChatSubmissionPolicy.hasContent(prompt: model.draft, attachmentCount: model.pendingAttachments.count))
                        .help("送信 (Enter、Shift+Enter で改行)")
                }
            }
        }
        .padding(10).background(Color(nsColor: .textBackgroundColor)).clipShape(RoundedRectangle(cornerRadius: 7))
        .overlay(RoundedRectangle(cornerRadius: 7).stroke(Color(nsColor: .separatorColor), lineWidth: 0.7))
        .frame(maxWidth: 760).padding(.horizontal, 22).padding(.top, 10).padding(.bottom, 14)
        .frame(maxWidth: .infinity).background(Color(nsColor: .windowBackgroundColor))
        .onAppear {
            model.reloadCompletionHosts()
            isChatInputFocused = !model.isWorking
        }
        .onChange(of: model.isWorking) { isWorking in
            if !isWorking { isChatInputFocused = true }
        }

        .onChange(of: hostSuggestions.map(\.hostname)) { _ in
            hostSuggestionIndex = min(hostSuggestionIndex, max(0, hostSuggestions.count - 1))
        }
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
                Button("旧形式 JSON から取り込む") { importLegacyRegistry() }
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
                        Text(connection.sourceID == nil ? "Mac 内" : "Imported")
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
                Text("資格情報は macOS Keychain に暗号化保存されます。CSV 形式での入出力や 旧形式の機器メタデータ取り込みに対応しています。")
                    .font(.system(size: 11)).foregroundStyle(.secondary)
                Spacer()
                Button("CSV を読み込む") { importCSV() }
                Button("CSV を書き出す") { exportCSV() }.disabled(model.connections.isEmpty)
            }.padding(12).background(Color(nsColor: .controlBackgroundColor).opacity(0.5))
        }
        .alert("機器情報", isPresented: $showsCSVAlert) { Button("OK", role: .cancel) {} } message: { Text(csvAlert) }
    }

    private func importLegacyRegistry() {
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
        let result: LegacyConnectionImportResult
        do {
            result = try model.importLegacyDevices(fromJSON: Data(json.utf8))
        } catch {
            presentCSVMessage("旧形式の機器情報 JSON を読み取れませんでした。元ファイルは変更していません。")
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
            presentCSVMessage("\(invalid.name) のホスト名が 旧 CSV 形式の制約に合いません。機器情報を編集してください。")
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
                WorkspaceTabButton(title: tab.rawValue, icon: tab.icon, isSelected: model.selectedToolTab == tab) {
                    model.selectedToolTab = tab
                }
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

private struct WorkspaceTabButton: View {
    let title: String
    let icon: String
    let isSelected: Bool
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(spacing: 6) {
                Image(systemName: icon)
                Text(title)
            }
            .font(.system(size: 12, weight: isSelected ? .semibold : .regular))
            .padding(.horizontal, 12).padding(.vertical, 7)
            .background(isSelected ? Color(nsColor: .selectedControlColor).opacity(0.18) : .clear, in: RoundedRectangle(cornerRadius: 6))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }
}

// MARK: - Full Settings Workspace

private struct RightAlignedSwitchStyle: ToggleStyle {
    func makeBody(configuration: Configuration) -> some View {
        HStack(spacing: 16) {
            configuration.label
                .frame(maxWidth: .infinity, alignment: .leading)
            Toggle(isOn: configuration.$isOn) {
                configuration.label
            }
            .labelsHidden()
            .toggleStyle(.switch)
            .fixedSize()
        }
        .frame(maxWidth: .infinity)
    }
}

private struct MonitoringWorkspace: View {
    @ObservedObject var model: DesktopModel
    @State private var selectedTab = 0

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack {
                Text("監視と実行履歴").font(.system(size: 22, weight: .semibold))
                Spacer()
                Text(model.watchStatus).font(.system(size: 12)).foregroundStyle(.secondary).lineLimit(2)
                Button("更新") { model.refreshWatches(); model.refreshAgentTasks() }
            }
            Picker("表示", selection: $selectedTab) {
                Text("CPU監視").tag(0)
                Text("Agent履歴").tag(1)
                Text("操作監査").tag(2)
            }.pickerStyle(.segmented).frame(maxWidth: 280)
            if selectedTab == 0 { watchContent }
            else if selectedTab == 1 { taskContent }
            else { operationAuditContent }
        }
        .padding(24)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .onAppear { model.refreshWatches(); model.refreshAgentTasks(); model.refreshOperationAudit() }
    }

    private var watchContent: some View {
        VStack(alignment: .leading, spacing: 12) {
            GroupBox("新しいCPU監視") {
                VStack(alignment: .leading, spacing: 10) {
                    HStack {
                        TextField("監視名", text: $model.watchName)
                        Picker("機器", selection: $model.watchDevice) {
                            Text("機器を選択").tag("")
                            ForEach(model.connections) { connection in Text(connection.name).tag(connection.name) }
                        }.frame(width: 220)
                    }
                    HStack {
                        TextField("間隔（秒）", text: $model.watchInterval).frame(width: 130)
                        TextField("CPUしきい値（%）", text: $model.watchThreshold).frame(width: 180)
                        TextField("通知メッセージ", text: $model.watchMessage)
                        if model.watchEditingID != nil { Button("取消") { model.watchEditingID = nil } }
                        Button(model.watchEditingID == nil ? "監視を作成" : "監視を更新") { model.createCPUWatch() }.buttonStyle(.borderedProminent)
                    }
                    Text("読み取り専用のCPU状態を定期確認し、しきい値を超えたときに通知します。操作や設定変更は実行しません。")
                        .font(.caption).foregroundStyle(.secondary)
                }.padding(.vertical, 4)
            }
            ScrollView {
                VStack(alignment: .leading, spacing: 10) {
                    ForEach(Array(model.watches.enumerated()), id: \.offset) { _, watch in
                        watchCard(watch)
                    }
                    if model.watches.isEmpty {
                        VStack(spacing: 8) {
                            Image(systemName: "waveform.path.ecg").font(.title2).foregroundStyle(.secondary)
                            Text("監視設定はありません").font(.headline)
                            Text("機器とCPUしきい値を指定して監視を作成できます。").font(.caption).foregroundStyle(.secondary)
                        }.frame(maxWidth: .infinity).padding(28)
                    }
                }
            }
        }
    }

    private func watchCard(_ watch: NativeWatch) -> some View {
        GroupBox {
            VStack(alignment: .leading, spacing: 8) {
                HStack {
                    VStack(alignment: .leading, spacing: 3) {
                        Text(watch.name).font(.headline)
                        Text("\(watch.status == "enabled" ? "有効" : "停止中") · \(watch.ir.schedule.every) · \(watchDeviceName(watch))")
                            .font(.caption).foregroundStyle(.secondary)
                    }
                    Spacer()
                    Button("今すぐ実行") { model.runWatch(watch) }
                    Button("編集") { model.editWatch(watch) }
                    Button(watch.status == "enabled" ? "停止" : "再開") { model.setWatch(watch, enabled: watch.status != "enabled") }
                    Button(role: .destructive) { model.deleteWatch(watch) } label: { Image(systemName: "trash") }
                }
                if let error = watch.lastError { Label(error, systemImage: "exclamationmark.triangle").font(.caption).foregroundStyle(.orange) }
                if let latest = watch.history?.last {
                    Text("直近: \(latest.completedAt) · \(latest.error ?? (latest.notifications.isEmpty ? "通知なし" : latest.notifications.map(\.message).joined(separator: "、")))")
                        .font(.caption).foregroundStyle(latest.error == nil ? Color.secondary : Color.orange)
                }
            }.frame(maxWidth: .infinity, alignment: .leading)
        }
    }

    private func watchDeviceName(_ watch: NativeWatch) -> String {
        for step in watch.ir.steps {
            if case .call(let call) = step { return call.args.device }
        }
        return ""
    }

    private var taskContent: some View {
        HStack(alignment: .top, spacing: 14) {
            List(selection: $model.selectedAgentTaskID) {
                ForEach(model.agentTasks) { task in
                    VStack(alignment: .leading, spacing: 4) {
                        Text(task.goal).lineLimit(2)
                        Text("\(task.status) · \(task.eventCount)件 · \(task.lastEventAt)").font(.caption).foregroundStyle(.secondary)
                    }.tag(task.id)
                }
            }.frame(minWidth: 250)
            .onChange(of: model.selectedAgentTaskID) { _ in
                guard let id = model.selectedAgentTaskID, let task = model.agentTasks.first(where: { $0.id == id }) else { return }
                model.loadAgentTaskHistory(task)
            }
            VStack(alignment: .leading, spacing: 10) {
                HStack {
                    Text("選択したタスクの記録").font(.headline)
                    Spacer()
                    if let selected = model.agentTasks.first(where: { $0.id == model.selectedAgentTaskID }) {
                        Button("調査を再開") { model.resumeAgentTask(selected) }
                    }
                }
                ScrollView { Text(model.selectedTaskHistory).font(.system(size: 11, design: .monospaced)).textSelection(.enabled).frame(maxWidth: .infinity, alignment: .leading) }
                    .padding(8).background(Color(nsColor: .textBackgroundColor), in: RoundedRectangle(cornerRadius: 8))
            }.frame(maxWidth: .infinity, maxHeight: .infinity)
        }
    }

    private var operationAuditContent: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Text("承認済み操作の監査記録").font(.headline)
                Spacer()
                Button("更新") { model.refreshOperationAudit() }
            }
            ScrollView {
                Text(model.operationAuditText.isEmpty ? "記録はありません" : model.operationAuditText)
                    .font(.system(size: 11, design: .monospaced)).textSelection(.enabled)
                    .frame(maxWidth: .infinity, alignment: .leading).padding(12)
            }.background(Color(nsColor: .textBackgroundColor), in: RoundedRectangle(cornerRadius: 8))
        }.frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

private struct SettingsWorkspace: View {
    @ObservedObject var model: DesktopModel
    @State private var selectedCategory = 0
    @State private var availablePorts: [String] = []

    private let categories: [(title: String, icon: String, color: Color)] = [
        ("チャット・通信", "bubble.left.and.bubble.right.fill", .blue),
        ("LLM モデル", "cpu", .purple),
        ("Vision (画像)", "photo.fill", .pink),
        ("ナレッジ RAG", "books.vertical.fill", .orange),
        ("設定", "arrow.triangle.2.circlepath", .green)
    ]

    var body: some View {
        HStack(spacing: 0) {
            VStack(alignment: .leading, spacing: 6) {
                Text("設定")
                    .font(.system(size: 11, weight: .semibold))
                    .foregroundStyle(.secondary)
                    .padding(.horizontal, 12).padding(.bottom, 8)
                ForEach(categories.indices, id: \.self) { index in
                    Button { selectedCategory = index } label: {
                        HStack(spacing: 10) {
                            Image(systemName: categories[index].icon)
                                .font(.system(size: 13, weight: .medium))
                                .foregroundStyle(.white)
                                .frame(width: 26, height: 26)
                                .background(categories[index].color.gradient, in: RoundedRectangle(cornerRadius: 6))
                            Text(categories[index].title)
                                .font(.system(size: 13, weight: selectedCategory == index ? .semibold : .regular))
                            Spacer(minLength: 0)
                        }
                        .foregroundStyle(selectedCategory == index ? Color.white : Color.primary)
                        .padding(.horizontal, 10).padding(.vertical, 8)
                        .background(selectedCategory == index ? Color.accentColor : .clear, in: RoundedRectangle(cornerRadius: 8))
                        .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                }
                Spacer()
            }
            .padding(12)
            .frame(width: 210)
            .frame(maxHeight: .infinity)
            .background(.regularMaterial)

            Divider()

            ScrollView {
                VStack(alignment: .leading, spacing: 18) {
                    Text(categories[selectedCategory].title)
                        .font(.system(size: 22, weight: .bold))
                        .padding(.bottom, 4)
                    VStack(alignment: .leading, spacing: 20) {
                        switch selectedCategory {
                        case 0: chatAndNetworkSection
                        case 1: llmModelSection
                        case 2: visionSection
                        case 3: knowledgeSection
                        case 4: nativeSettingsSection
                        default: EmptyView()
                        }
                    }
                    .padding(20)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(Color(nsColor: .controlBackgroundColor), in: RoundedRectangle(cornerRadius: 12))
                    .overlay {
                        RoundedRectangle(cornerRadius: 12)
                            .strokeBorder(Color(nsColor: .separatorColor).opacity(0.5), lineWidth: 1)
                    }
                }
                .padding(28)
                .frame(maxWidth: 880, alignment: .leading)
                .frame(maxWidth: .infinity, alignment: .topLeading)
            }
            .background(Color(nsColor: .underPageBackgroundColor))
        }
        .toggleStyle(RightAlignedSwitchStyle())
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
                    set: { model.settings.historyLimit = Int($0); model.saveSettings() }
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
                    set: { model.settings.temperature = $0; model.saveSettings() }
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
                    set: { model.settings.repetitionPenalty = $0; model.saveSettings() }
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
                    set: { model.settings.mcpTimeout = Int($0); model.saveSettings() }
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
                    set: { model.settings.cacheExpiryMinutes = Int($0); model.saveSettings() }
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
                    set: { model.settings.ipVersion = $0; model.saveSettings() }
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
                set: { model.settings.autoDryRun = $0; model.saveSettings() }
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
                        set: { model.settings.consolePort = $0.isEmpty ? nil : $0; model.saveSettings() }
                    )) {
                        Text("未設定 (None)").tag("")
                        ForEach(availablePorts, id: \.self) { port in
                            Text(port).tag(port)
                        }
                    }
                    .frame(maxWidth: .infinity)

                    Picker("ボーレート", selection: Binding(
                        get: { model.settings.consoleBaudRate ?? 9600 },
                        set: { model.settings.consoleBaudRate = $0; model.saveSettings() }
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
                            model.saveSettings()
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
                            set: { model.settings.nCtx = $0; model.saveSettings() }
                        ), formatter: NumberFormatter())
                        .textFieldStyle(.roundedBorder)
                        .frame(width: 100)
                    }

                    VStack(alignment: .leading, spacing: 4) {
                        Text("最大生成トークン (max_gen)").font(.system(size: 11))
                        TextField("2048", value: Binding(
                            get: { model.settings.maxGen },
                            set: { model.settings.maxGen = $0; model.saveSettings() }
                        ), formatter: NumberFormatter())
                        .textFieldStyle(.roundedBorder)
                        .frame(width: 100)
                    }

                    VStack(alignment: .leading, spacing: 4) {
                        Text("保持トークン数 (prompt_keep)").font(.system(size: 11))
                        TextField("500", value: Binding(
                            get: { model.settings.promptKeepTokens },
                            set: { model.settings.promptKeepTokens = $0; model.saveSettings() }
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

                VStack(alignment: .leading, spacing: 12) {
                    Toggle("ナレッジワーカー", isOn: Binding(
                        get: { model.settings.preloadKnowledge },
                        set: { model.settings.preloadKnowledge = $0; model.saveSettings() }
                    ))
                    Divider()
                    Toggle("アナリストワーカー", isOn: Binding(
                        get: { model.settings.preloadAnalysis },
                        set: { model.settings.preloadAnalysis = $0; model.saveSettings() }
                    ))
                    Divider()
                    Toggle("RAG ワーカー", isOn: Binding(
                        get: { model.settings.preloadRag },
                        set: { model.settings.preloadRag = $0; model.saveSettings() }
                    ))
                    Divider()
                    Toggle("ビルダーワーカー", isOn: Binding(
                        get: { model.settings.preloadBuilder },
                        set: { model.settings.preloadBuilder = $0; model.saveSettings() }
                    ))
                    Divider()
                    Toggle("プロッターワーカー", isOn: Binding(
                        get: { model.settings.preloadPlotter },
                        set: { model.settings.preloadPlotter = $0; model.saveSettings() }
                    ))
                    Divider()
                    Toggle("要約ワーカー", isOn: Binding(
                        get: { model.settings.preloadSummarization },
                        set: { model.settings.preloadSummarization = $0; model.saveSettings() }
                    ))
                }
                .toggleStyle(RightAlignedSwitchStyle())
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(16)
                .background(Color(nsColor: .windowBackgroundColor).opacity(0.55), in: RoundedRectangle(cornerRadius: 10))
                .overlay {
                    RoundedRectangle(cornerRadius: 10)
                        .strokeBorder(Color(nsColor: .separatorColor).opacity(0.45), lineWidth: 1)
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
                set: { model.settings.visionEnabled = $0; model.saveSettings() }
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
                        set: { model.settings.mmprojPath = $0.isEmpty ? nil : $0; model.saveSettings() }
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
                            model.saveSettings()
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

    // MARK: Category 4: Native Settings

    private var nativeSettingsSection: some View {
        VStack(alignment: .leading, spacing: 18) {
            Text("Swift版設定").font(.system(size: 15, weight: .semibold))

            VStack(alignment: .leading, spacing: 10) {
                HStack(spacing: 8) {
                    Circle().fill(model.isSettingsLoaded ? Color.green : Color.orange).frame(width: 10, height: 10)
                    Text(model.isSettingsLoaded ? "設定を読み込み済み" : "デフォルト設定を使用中")
                        .font(.system(size: 13, weight: .semibold))
                        .foregroundStyle(model.isSettingsLoaded ? Color.green : Color.orange)
                    Spacer()
                }

                Text(model.settingsStatusMessage)
                    .font(.system(size: 12, design: .monospaced))
                    .foregroundStyle(.secondary)
                    .textSelection(.enabled)
                    .padding(10)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(Color(nsColor: .controlBackgroundColor), in: RoundedRectangle(cornerRadius: 6))

                HStack(spacing: 10) {
                    Button("設定を再読込") {
                        model.loadSettings()
                    }
                    .buttonStyle(.bordered)

                    Button("設定を保存") {
                        model.saveSettings()
                    }
                    .buttonStyle(.borderedProminent)

                    Button("設定フォルダを Finder で開く") {
                        model.openSettingsDirectory()
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
                Text("設定は Swift版のApplication Support配下へ保存され、起動時に読み込まれます。")
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
