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

    private let defaults: UserDefaults
    private let credentialPersistence = ConnectionCredentialPersistence(store: KeychainCredentialAdapter())
    private let sessionsKey = "mikomai.desktop.mac.sessions.v1"
    private let activeKey = "mikomai.desktop.mac.activeSession.v1"
    private let connectionsKey = "mikomai.desktop.mac.connections.v1"

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
        let raw = response.message.map { String(cString: $0) } ?? "タスク履歴を読み込めませんでした"
        selectedTaskHistory = AgentTaskHistoryPresentation.lines(from: raw, fallbackGoal: task.goal).joined(separator: "\n\n")
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
        if activeSessionID == nil || !sessions.contains(where: { $0.id == activeSessionID }) {
            createSession()
        }
        guard let id = activeSessionID else { return }
        chatQueue.enqueue(QueuedChatSubmission(sessionID: id, prompt: prompt, attachments: pendingAttachments))
        pendingAttachments = []
        attachmentError = ""
        draft = ""
        startNextQueuedSubmission()
    }

    func removeQueuedSubmission(_ id: UUID) { chatQueue.remove(id: id) }

    private func startNextQueuedSubmission() {
        guard let submission = chatQueue.takeNext(isWorking: isWorking, validSessionIDs: Set(sessions.map(\.id))) else { return }
        submit(submission)
    }

    private func submit(_ submission: QueuedChatSubmission) {
        let id = submission.sessionID
        let prompt = submission.prompt
        let attachments = submission.attachments
        guard let index = sessions.firstIndex(where: { $0.id == id }) else { return }
        let recentHostCandidates = Self.recentHostCandidates(in: prompt)
        if !recentHostCandidates.isEmpty {
            let updated = HostSuggestionPolicy.updateRecentHosts(recentHostCandidates, current: settings.recentIps)
            if updated != settings.recentIps {
                settings.recentIps = updated
                saveSettings()
            }
        }

        // Context limit derived from settings.historyLimit
        let maxHistoryTurns = max(2, settings.historyLimit * 2)
        let history = sessions[index].messages.suffix(maxHistoryTurns).map { message in
            "\(message.role == .user ? "ユーザー" : "MIKOMAI"): \(message.text)"
        }.joined(separator: "\n")

        let attachedNames = attachments.map(\.name)
        let userText = prompt.isEmpty ? "添付ファイルを確認してください。" : prompt
        let submissionText: String
        if let taskID = pendingSavedAgentTaskIDs.removeValue(forKey: id) {
            submissionText = "__MIKOMAI_RESUME_SAVED__\(taskID)"
        } else if let taskID = pendingAgentTaskIDs.removeValue(forKey: id) {
            submissionText = "__MIKOMAI_RESUME__\(taskID)\n\(userText)"
        } else {
            submissionText = userText
        }
        let attachmentText = attachments.enumerated().map { offset, attachment in
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
        guard let requestID = chatResponse.begin(sessionID: id) else { return }
        let isAgentRequest = Self.dispatchMode(submissionText, connections: agentConnections) == "agent"
        if isAgentRequest, let messageIndex = sessions[index].messages.firstIndex(where: { $0.id == assistantID }) {
            sessions[index].messages[messageIndex].agentGoal = userText
            sessions[index].messages[messageIndex].agentProgress = [AgentProgressEntry(phase: "準備", nextAction: "実行環境を確認して計画を作成", detail: "Agentを起動しています")]
        }

        Task.detached(priority: .userInitiated) {
            // Auto-load model if configured but not yet loaded in Rust FFI
            let currentLoaded = Self.callRust { mikomai_model_status() }
            if currentLoaded.isEmpty && !modelP.isEmpty && FileManager.default.fileExists(atPath: modelP) {
                if isAgentRequest {
                    await MainActor.run {
                        guard self.chatResponse.acceptsChunk(for: requestID),
                              let sIdx = self.sessions.firstIndex(where: { $0.id == id }),
                              let mIdx = self.sessions[sIdx].messages.firstIndex(where: { $0.id == assistantID }) else { return }
                        self.sessions[sIdx].messages[mIdx].agentProgress?.append(AgentProgressEntry(phase: "モデル準備", nextAction: "モデルを読み込んで調査を開始", detail: "ローカルモデルを読み込んでいます"))
                    }
                }
                _ = Self.callRust { modelP.withCString { mikomai_model_load($0) } }
                await MainActor.run {
                    self.refreshModelStatus()
                }
            }

            let shouldRun = await MainActor.run {
                guard self.chatResponse.requestID == requestID else { return false }
                if self.isCancelling {
                    if let sIdx = self.sessions.firstIndex(where: { $0.id == id }),
                       let mIdx = self.sessions[sIdx].messages.firstIndex(where: { $0.id == assistantID }) {
                        self.sessions[sIdx].messages[mIdx].text = "生成を停止しました。"
                    }
                    self.chatResponse.finish(requestID)
                    self.startNextQueuedSubmission()
                    return false
                }
                return true
            }
            guard shouldRun else { return }

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
                        guard self.chatResponse.acceptsChunk(for: requestID),
                              let plan = try? JSONDecoder().decode(NativeOperationPlan.self, from: data) else { return }
                        if let sIdx = self.sessions.firstIndex(where: { $0.id == id }),
                           let mIdx = self.sessions[sIdx].messages.firstIndex(where: { $0.id == assistantID }) {
                            self.sessions[sIdx].messages[mIdx].agentGoal = userText
                            self.sessions[sIdx].messages[mIdx].agentProgress = (self.sessions[sIdx].messages[mIdx].agentProgress ?? []) + [AgentProgressEntry(phase: "承認待ち", nextAction: "変更計画を確認して承認", detail: plan.rationale)]
                        }
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
                },
                onToolResult: { result in
                    Task { @MainActor in
                        guard self.sessions.contains(where: { $0.id == id }) else { return }
                        var result = result
                        result.sessionID = id
                        if result.isLocalProbe {
                            self.executionResults.append(result)
                            self.executionResults = Array(self.executionResults.suffix(40))
                        } else {
                            self.recentToolResults.insert(result, at: 0)
                            self.recentToolResults = Array(self.recentToolResults.prefix(8))
                        }
                    }
                }
            ) { chunk, _ in
                Task { @MainActor in
                    guard self.chatResponse.acceptsChunk(for: requestID),
                          let sIdx = self.sessions.firstIndex(where: { $0.id == id }),
                          let mIdx = self.sessions[sIdx].messages.firstIndex(where: { $0.id == assistantID }) else { return }
                    if let progress = AgentProgressEntry.parse(chunk) {
                        self.sessions[sIdx].messages[mIdx].agentGoal = userText
                        self.sessions[sIdx].messages[mIdx].agentProgress = (self.sessions[sIdx].messages[mIdx].agentProgress ?? []) + [progress]
                    } else if !chunk.hasPrefix(AgentProgressEntry.streamPrefix) {
                        self.sessions[sIdx].messages[mIdx].text += chunk
                    }
                    self.sessions[sIdx].updatedAt = Date()
                }
            }
            await MainActor.run {
                guard self.chatResponse.requestID == requestID else { return }
                defer {
                    self.chatResponse.finish(requestID)
                    self.startNextQueuedSubmission()
                }
                guard let sIdx = self.sessions.firstIndex(where: { $0.id == id }),
                      let mIdx = self.sessions[sIdx].messages.firstIndex(where: { $0.id == assistantID }) else {
                    return
                }
                var displayAnswer = finalAnswer
                if !self.isCancelling, finalAnswer.hasPrefix("__MIKOMAI_CHOICE__"),
                   let payload = finalAnswer.dropFirst("__MIKOMAI_CHOICE__".count).data(using: .utf8),
                   let choice = try? JSONSerialization.jsonObject(with: payload) as? [String: Any],
                   let taskID = choice["task_id"] as? String,
                   let text = choice["text"] as? String {
                    self.pendingAgentTaskIDs[id] = taskID
                    displayAnswer = text
                    if let options = choice["question"] as? [String: Any],
                       let values = options["options"] as? [String], !values.isEmpty {
                        displayAnswer += "\n\n" + values.enumerated().map { "\($0.offset + 1). \($0.element)" }.joined(separator: "\n")
                    }
                }
                if self.sessions[sIdx].messages[mIdx].agentProgress != nil {
                    let phase: String
                    let nextAction: String
                    let detail: String
                    if self.isCancelling {
                        phase = "停止"; nextAction = "必要に応じて再開"; detail = "生成を停止しました"
                    } else if finalAnswer.hasPrefix("エラー:") {
                        phase = "失敗"; nextAction = "エラー内容と接続設定を確認"; detail = displayAnswer
                    } else if self.pendingAgentTaskIDs[id] != nil || displayAnswer.hasPrefix("### ❓ 確認要求") {
                        phase = "確認待ち"; nextAction = "確認事項に回答"; detail = "追加の情報が必要です"
                    } else if displayAnswer.hasPrefix("### ✅ 承認待ち") {
                        phase = "承認待ち"; nextAction = "変更計画を確認して承認"; detail = "承認後に変更を実行できます"
                    } else {
                        phase = "完了"; nextAction = "回答と実行結果を確認"; detail = "調査結果を回答にまとめました"
                    }
                    self.sessions[sIdx].messages[mIdx].agentProgress?.append(AgentProgressEntry(phase: phase, nextAction: nextAction, detail: detail))
                }
                // The FFI result is authoritative. Reporter status and queued
                // chunks must never remain in the saved final answer.
                self.sessions[sIdx].messages[mIdx].text = self.chatResponse.finalText(
                    streamed: self.sessions[sIdx].messages[mIdx].text, answer: displayAnswer
                )
                self.sessions[sIdx].updatedAt = Date()
                self.refreshAgentTasks()
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
        chatResponse.cancel()
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

    nonisolated static func runPortableAgentTool(
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
                let commandText = ([process.executableURL!.path] + (process.arguments ?? [])).joined(separator: " ")
                let out = Pipe(); let err = Pipe()
                process.standardOutput = out; process.standardError = err
                do {
                    try process.run(); process.waitUntilExit()
                    let stdout = String(decoding: out.fileHandleForReading.readDataToEndOfFile(), as: UTF8.self)
                    let stderr = String(decoding: err.fileHandleForReading.readDataToEndOfFile(), as: UTF8.self)
                    return NetworkOperationOutput(success: process.terminationStatus == 0, stdout: stdout, stderr: stderr, command: commandText)
                } catch { return NetworkOperationOutput(success: false, stdout: "", stderr: error.localizedDescription, command: commandText) }
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

    private nonisolated static func publicDevicesJSON(_ connections: [SavedConnection]) -> String {
        let devices = connections.map { connection in
            ["id": connection.id.uuidString, "hostname": connection.name, "ip": connection.host, "deviceType": connection.deviceType]
        }
        return (try? JSONSerialization.data(withJSONObject: devices)).map { String(decoding: $0, as: UTF8.self) } ?? "[]"
    }

    private nonisolated static func dispatchMode(_ prompt: String, connections: [SavedConnection]) -> String {
        let devices = publicDevicesJSON(connections)
        return prompt.withCString { message in
            devices.withCString { targets in
                let result = mikomai_dispatch_mode(message, targets)
                defer { mikomai_result_free(result) }
                return result.message.map { String(cString: $0) } ?? "worker"
            }
        }
    }

    private nonisolated static func askRustStreaming(
        _ prompt: String,
        history: String,
        documents: String,
        knowledge: String,
        attachments: String,
        connections: [SavedConnection],
        credentialPersistence: ConnectionCredentialPersistence,
        onOperationPlan: @escaping (Data) -> Void,
        onToolResult: @escaping (AgentToolResult) -> Void,
        onChunk: @escaping (String, Bool) -> Void
    ) -> String {
        let box = ChatCallbackBox(stream: StreamBox(onChunk: onChunk), connections: connections, credentialPersistence: credentialPersistence, onOperationPlan: onOperationPlan, onToolResult: onToolResult)
        let context = Unmanaged.passUnretained(box).toOpaque()
        let devicesJSON = Self.publicDevicesJSON(connections)
        let mode = Self.dispatchMode(prompt, connections: connections)
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

    nonisolated static func executeApprovedAgentOperation(planID: String, planHash: String, password: String?) -> NetworkOperationOutput {
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
