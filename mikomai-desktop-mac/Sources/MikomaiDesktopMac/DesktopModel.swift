import SwiftUI
import AppKit
import Foundation
import Darwin
import Security
import CryptoKit
import MikomaiFFI
import MikomaiDesktopCore
import UniformTypeIdentifiers

// MARK: - DesktopModel

@MainActor
final class DesktopModel: ObservableObject {
    @Published var debugRecords: [CoreDebugRecord] = []
    @Published var workspace: Workspace = .chat
    @Published var selectedToolTab: ToolTab = .tcpTest
    @Published var sessions: [ChatSession] = [] { didSet { persistSessions() } }
    @Published var activeSessionID: UUID? { didSet { persistActiveSession() } }
    @Published var draft = ""
    @Published var pendingAttachments: [PendingAttachment] = []
    @Published var attachmentError = ""
    @Published var chatResponse = ChatResponseLifecycle()
    var isWorking: Bool { chatResponse.isWorking }
    var isWorkingInActiveSession: Bool { chatResponse.showsProgress(in: activeSessionID) }
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
    lazy var operationCoordinator: OperationCoordinator = OperationCoordinator(model: self)
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
    @Published var selectedTaskHistory: [AgentTaskHistoryItem] = []
    @Published var chatQueue = ChatSubmissionQueue()
    @Published var executionResults: [AgentToolResult] = []
    var queuedSubmissionsInActiveSession: [QueuedChatSubmission] {
        chatQueue.submissions.filter { $0.sessionID == activeSessionID }
    }
    var executionResultsInActiveSession: [AgentToolResult] {
        executionResults.filter { $0.sessionID == activeSessionID }
    }
    @Published var recentToolResults: [AgentToolResult] = []
    @Published var operationAuditText = ""
    @Published var selectedAgentTaskID: String?
    var watchCallbackBox: WatchCallbackBox?
    var watchCallbackContext: UnsafeMutableRawPointer?
    var pendingAgentTaskIDs: [UUID: String] = [:]
    var pendingSavedAgentTaskIDs: [UUID: String] = [:]

    // Knowledge dirs
    @Published var documentsDirectory: String { didSet { defaults.set(documentsDirectory, forKey: "mikomai.desktop.mac.documentsDirectory") } }
    @Published var knowledgeDirectory: String { didSet { defaults.set(knowledgeDirectory, forKey: "mikomai.desktop.mac.knowledgeDirectory") } }

    // Model path & status
    @Published var modelPath: String = "" { didSet { defaults.set(modelPath, forKey: "mikomai.desktop.mac.modelPath") } }
    @Published var modelStatus = "モデル未ロード"
    @Published var isLoadingModel = false
    var isAppleModelSelected: Bool { settings.llmBackend == .apple }
    var supportsAppleModelOS: Bool {
        AppleModelPolicy.supportsOS(majorVersion: ProcessInfo.processInfo.operatingSystemVersion.majorVersion)
    }
    var isCancelling: Bool { chatResponse.isCancelling }

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

    let defaults: UserDefaults
    let credentialPersistence = ConnectionCredentialPersistence(store: KeychainCredentialAdapter())
    let sessionsKey = "mikomai.desktop.mac.sessions.v1"
    let activeKey = "mikomai.desktop.mac.activeSession.v1"
    let connectionsKey = "mikomai.desktop.mac.connections.v1"

    init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
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
        let raw = response.message.map { String(cString: $0) } ?? "タスク履歴を読み込めませんでした"
        selectedTaskHistory = AgentTaskHistoryPresentation.items(from: raw, fallbackGoal: task.goal)
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
        chatQueue.remove(sessionID: id)
        executionResults.removeAll { $0.sessionID == id }
        pendingAgentTaskIDs.removeValue(forKey: id)
        pendingSavedAgentTaskIDs.removeValue(forKey: id)
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

    // MARK: - Operation Plans

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
        let deviceType = DeviceTypeCatalog.canonicalID(for: connection.deviceType)
        return NetworkRunnerRequest(
            action: action, host: connection.host, username: connection.username,
            password: credentials.password ?? "", secret: credentials.enablePassword ?? "",
            deviceType: connection.transportDeviceType(deviceType), port: connection.effectivePort, commands: commands
        )
    }

    // MARK: - Persistence


    func persistSessions() {
        guard let data = try? JSONEncoder().encode(sessions) else { return }
        defaults.set(data, forKey: sessionsKey)
    }

    func persistActiveSession() { defaults.set(activeSessionID?.uuidString, forKey: activeKey) }
    func persistConnections() {
        guard let data = try? JSONEncoder().encode(connections) else { return }
        defaults.set(data, forKey: connectionsKey)
    }


    // MARK: - Rust FFI Calls

    nonisolated static func publicDevicesJSON(_ connections: [SavedConnection]) -> String {
        let devices = connections.map { connection in
            ["id": connection.id.uuidString, "hostname": connection.name, "ip": connection.host, "deviceType": connection.deviceType]
        }
        return (try? JSONSerialization.data(withJSONObject: devices)).map { String(decoding: $0, as: UTF8.self) } ?? "[]"
    }

    nonisolated static func dispatchMode(_ prompt: String, connections: [SavedConnection]) -> String {
        let devices = publicDevicesJSON(connections)
        return prompt.withCString { message in
            devices.withCString { targets in
                let result = mikomai_dispatch_mode(message, targets)
                defer { mikomai_result_free(result) }
                return result.message.map { String(cString: $0) } ?? "worker"
            }
        }
    }

    nonisolated static func askRustStreaming(
        _ prompt: String,
        history: String,
        documents: String,
        knowledge: String,
        attachments: String,
        connections: [SavedConnection],
        credentialPersistence: ConnectionCredentialPersistence,
        onOperationPlan: @escaping (Data) -> Void,
        onDebug: @escaping (String) -> Void,
        onToolResult: @escaping (AgentToolResult) -> Void,
        onChunk: @escaping (String, Bool) -> Void
    ) -> String {
        let box = ChatCallbackBox(stream: StreamBox(onChunk: onChunk), connections: connections, credentialPersistence: credentialPersistence, onOperationPlan: onOperationPlan, onToolResult: onToolResult, onDebug: onDebug)
        let context = Unmanaged.passUnretained(box).toOpaque()
        let devicesJSON = Self.publicDevicesJSON(connections)
        let mode = Self.dispatchMode(prompt, connections: connections)
        onDebug(CoreDebugRecord.encode(kind: "swift_request", payload: ["query":prompt, "history":history, "attachments":attachments, "devices_json":devicesJSON, "mode":mode, "documents":documents, "knowledge":knowledge]))
        let response = prompt.withCString { message in
            devicesJSON.withCString { devices in
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
        onDebug(CoreDebugRecord.encode(kind: "core_response", payload: ["status":response.status, "text":text]))
        return response.status == 0 ? text : "エラー: \(text)"
    }

    nonisolated static func testTCP(host: String, port: UInt16, timeoutMs: UInt32) -> (success: Bool, message: String, latencyMs: Int?) {
        MikomaiFFIBridge.testTCP(host: host, port: port, timeoutMs: timeoutMs)
    }

    nonisolated static func callRust(_ call: () -> MikomaiResult) -> String {
        MikomaiFFIBridge.call(call).formattedOutput
    }

    nonisolated static func consumeRust(_ response: MikomaiResult) -> String {
        defer { mikomai_result_free(response) }
        guard let message = response.message else { return "応答がありませんでした。" }
        let text = String(cString: message)
        return response.status == 0 ? text : "エラー: \(text)"
    }


    nonisolated static func executeApprovedAgentOperation(planID: String, planHash: String, password: String?) -> NetworkOperationOutput {
        MikomaiFFIBridge.executeApprovedAgentOperation(planID: planID, planHash: planHash, password: password)
    }
}
