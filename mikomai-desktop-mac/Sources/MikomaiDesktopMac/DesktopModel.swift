import SwiftUI
import AppKit
import Foundation
import Darwin
import Security
import CryptoKit
import MikomaiBindings
import MikomaiDesktopCore
import UniformTypeIdentifiers

// MARK: - DesktopModel

@MainActor
final class DesktopModel: ObservableObject {
    @Published var debugRecords: [CoreDebugRecord] = []
    @Published var workspace: Workspace = .chat
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
    @Published var selectedExecutionMessageID: UUID?
    var queuedSubmissionsInActiveSession: [QueuedChatSubmission] {
        chatQueue.submissions.filter { $0.sessionID == activeSessionID }
    }
    var executionResultsInActiveSession: [AgentToolResult] {
        Array((activeSession?.messages.flatMap { $0.probeResults ?? [] } ?? []).suffix(40))
    }
    var displayedExecutionResults: [AgentToolResult] {
        let results = executionResultsInActiveSession
        let selected = results.filter { $0.messageID == selectedExecutionMessageID }
        return selected.isEmpty ? results : selected
    }
    @Published var recentToolResults: [AgentToolResult] = []
    @Published var operationAuditText = ""
    @Published var selectedAgentTaskID: String?
    var watchCallbackBox: WatchCallbackBox?
    var watchCallbackContext: UnsafeMutableRawPointer?
    var pendingAgentTaskIDs: [UUID: String] = [:]
    var pendingSavedAgentTaskIDs: [UUID: String] = [:]

    // Knowledge dirs
    @Published var documentsDirectory: String { didSet { persistSettingsPaths() } }
    @Published var knowledgeDirectory: String { didSet { persistSettingsPaths() } }

    // Model path & status
    @Published var modelPath: String = "" { didSet { persistSettingsPaths() } }
    @Published var modelStatus = "モデル未ロード"
    @Published var isLoadingModel = false
    var isAppleModelSelected: Bool { settings.llmBackend == .apple }
    var supportsAppleModelOS: Bool {
        AppleModelPolicy.supportsOS(majorVersion: ProcessInfo.processInfo.operatingSystemVersion.majorVersion)
    }
    var activeRustTaskID: String?
    var isCancelling: Bool { chatResponse.isCancelling }

    // Native settings
    @Published var settings: AppSettings = AppSettings()
    @Published var settingsFileURL: URL = SettingsManager.settingsURL
    @Published var isSettingsLoaded: Bool = false
    @Published var settingsStatusMessage: String = ""

    // Model Presets & HuggingFace
    @Published var selectedPresetId: String = "gemma-4-e4b-ud"
    @Published var repoPath: String = "unsloth/gemma-4-E4B-it-GGUF"
    @Published var modelFilename: String = "gemma-4-E4B-it-UD-Q4_K_XL.gguf"
    @Published var isDownloadingModel: Bool = false
    @Published var downloadProgressText: String = ""

    var isRestoringPersistence = true
    var persistenceAvailable = true
    var settingsPersistenceAvailable = true
    @Published var persistenceError = ""

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
        let rawDocs = ProcessInfo.processInfo.environment["MIKOMAI_DOCS_DIR"] ?? defaultDocuments
        let rawKnowledge = ProcessInfo.processInfo.environment["MIKOMAI_KNOWLEDGE_DIR"] ?? defaultKnowledge
        documentsDirectory = (rawDocs as NSString).expandingTildeInPath
        knowledgeDirectory = (rawKnowledge as NSString).expandingTildeInPath

        do {
            if let state: ChatSessionState = try NativePersistence.load("sessions") {
                sessions = state.sessions
                activeSessionID = state.activeSessionID
            }
            if let saved: [SavedConnection] = try NativePersistence.load("connections") { connections = saved }
        } catch {
            persistenceAvailable = false
            persistenceError = "保存データを読み込めませんでした: \(error.localizedDescription)"
        }
        isRestoringPersistence = false
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
        if let selectedID = selectedAgentTaskID {
            if let selected = agentTasks.first(where: { $0.id == selectedID }) {
                loadAgentTaskHistory(selected)
            } else {
                selectedAgentTaskID = nil
                selectedTaskHistory = []
            }
        }
    }

    func deleteAgentTask(_ task: NativeAgentTask) {
        let response = task.id.withCString { mikomai_agent_task_delete($0) }
        defer { mikomai_result_free(response) }
        agentTasks.removeAll { $0.id == task.id }
        if selectedAgentTaskID == task.id {
            selectedAgentTaskID = nil
            selectedTaskHistory = []
        }
    }

    func deleteAllAgentTasks() {
        let response = mikomai_agent_task_delete_all()
        defer { mikomai_result_free(response) }
        agentTasks.removeAll()
        selectedAgentTaskID = nil
        selectedTaskHistory = []
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

    func rerunAgentTask(_ task: NativeAgentTask) {
        guard !isLoadingModel else { return }
        let response = task.id.withCString { mikomai_agent_task_history($0) }
        defer { mikomai_result_free(response) }
        let history = response.message.map { String(cString: $0) } ?? ""
        let prompt = AgentTaskHistoryPresentation.initialPrompt(from: history, fallbackGoal: task.goal)
        guard !ChatSubmissionPolicy.normalizedPrompt(prompt).isEmpty else { return }
        createSession()
        guard let id = activeSessionID else { return }
        // Submit a fresh request without resume IDs or unrelated composer attachments.
        chatQueue.enqueue(QueuedChatSubmission(sessionID: id, prompt: prompt, attachments: []))
        draft = ""
        startNextQueuedSubmission()
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
        connections.map { HostSuggestion(hostname: $0.name, ip: $0.host) }
    }

    // MARK: - Operation Plans

    func createOperationPlan(target: SavedConnection, proposal: String, rationale: String) -> String? {
        do {
            operationPlan = try NativeCommands.prepareOperation(id: target.id.uuidString, proposal: proposal, rationale: rationale)
            operationPhase = "計画作成済み"
            return nil
        } catch { return error.localizedDescription }
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

    // MARK: - Persistence


    func persistSessions() {
        guard !isRestoringPersistence && persistenceAvailable else { return }
        do {
            try NativePersistence.save(ChatSessionState(sessions: sessions, activeSessionID: activeSessionID), collection: "sessions")
        } catch { persistenceError = "会話を保存できませんでした: \(error.localizedDescription)" }
    }
    func persistActiveSession() { persistSessions() }
    func persistConnections() {
        guard !isRestoringPersistence && persistenceAvailable else { return }
        do { try NativePersistence.save(connections, collection: "connections") }
        catch { persistenceError = "接続情報を保存できませんでした: \(error.localizedDescription)" }
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
        onTaskID: @escaping (String) -> Void,
        onOperationPlan: @escaping (Data) -> Void,
        onDebug: @escaping (String) -> Void,
        onToolResult: @escaping (AgentToolResult) -> Void,
        onChunk: @escaping (String, Bool) -> Void
    ) -> String {
        let service = MikomaiService()
        do {
            let taskID = try service.submit(command: .chat(message:prompt,history:history,documentsDir:documents,knowledgeDir:knowledge,attachments:attachments,devicesJson:Self.publicDevicesJSON(connections),agent:true))
            onTaskID(taskID)
            var seq: UInt64 = 0
            while true {
                let snapshot = try service.query(query:.task(taskId:taskID))
                for event in snapshot.events where event.seq > seq {
                    seq = event.seq
                    guard let object = try? JSONSerialization.jsonObject(with:Data(event.payload.utf8)) as? [String:Any], let text = object["text"] as? String else { continue }
                    if text.hasPrefix("__MIKOMAI_DEBUG__") { onDebug(String(text.dropFirst("__MIKOMAI_DEBUG__".count))) }
                    else if text.hasPrefix("__MIKOMAI_APPROVAL_PLAN__") { onOperationPlan(Data(text.dropFirst("__MIKOMAI_APPROVAL_PLAN__".count).utf8)) }
                    else { onChunk(text,object["done"] as? Bool ?? false) }
                }
                if ["completed","awaiting_user","awaiting_approval","failed","cancelled","unknown"].contains(snapshot.state) {
                    return ["failed","unknown"].contains(snapshot.state) ? "エラー: \(snapshot.result)" : snapshot.result
                }
                Thread.sleep(forTimeInterval:0.01)
            }
        } catch { return "エラー: \(error.localizedDescription)" }
    }

    nonisolated static func callRust(_ call: () -> MikomaiResult) -> String { MikomaiFFIBridge.call(call).formattedOutput }

    nonisolated static func consumeRust(_ response: MikomaiResult) -> String {
        defer { mikomai_result_free(response) }
        guard let message = response.message else { return "応答がありませんでした。" }
        let text = String(cString: message)
        return response.status == 0 ? text : "エラー: \(text)"
    }



}
