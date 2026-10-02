import Foundation
import AppKit
import UniformTypeIdentifiers
import MikomaiDesktopCore
import MikomaiFFI

extension DesktopModel {
    // MARK: - Streaming Chat Operations

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

    func startNextQueuedSubmission() {
        guard let submission = chatQueue.takeNext(isWorking: isWorking, validSessionIDs: Set(sessions.map(\.id))) else { return }
        submit(submission)
    }

    func submit(_ submission: QueuedChatSubmission) {
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
        let attachmentText: String
        do { attachmentText = try NativeAttachmentPayload.encode(attachments) }
        catch { attachmentError = error.localizedDescription; return }

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
                    DispatchQueue.main.async {
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
                    DispatchQueue.main.async {
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
                DispatchQueue.main.async {
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
            // Use the same FIFO main queue as callback delivery. This both
            // wakes AppKit and applies all reported events before finalization.
            await withCheckedContinuation { (completion: CheckedContinuation<Void, Never>) in
                DispatchQueue.main.async {
                    defer { completion.resume() }
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
                    self.persistSessions()
                }
            }
            // Audit listing can perform disk/DB work; keep it off the UI queue
            // and outside the response lifecycle so completion paints promptly.
            let taskList = Self.callRust { mikomai_agent_task_list() }
            if let data = taskList.data(using: .utf8),
               let tasks = try? JSONDecoder().decode([NativeAgentTask].self, from: data) {
                DispatchQueue.main.async { self.agentTasks = tasks }
            }
        }
    }

    static func recentHostCandidates(in text: String) -> [String] {
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
            .plainText, .commaSeparatedText, .json, .yaml, .xml, .png, .jpeg,
            UTType(filenameExtension: "md") ?? .plainText,
            UTType(filenameExtension: "log") ?? .plainText
        ]
        panel.canChooseFiles = true
        panel.canChooseDirectories = false
        panel.allowsMultipleSelection = true
        guard panel.runModal() == .OK else { return }

        var loaded = pendingAttachments
        var totalBytes = loaded.filter { $0.imageData == nil }.reduce(0) { $0 + $1.byteCount }
        for url in panel.urls {
            guard !loaded.contains(where: { $0.name == url.lastPathComponent }) else { continue }
            let hasScope = url.startAccessingSecurityScopedResource()
            defer { if hasScope { url.stopAccessingSecurityScopedResource() } }
            do {
                let handle = try FileHandle(forReadingFrom: url)
                defer { try? handle.close() }
                let isImage = ImageAttachmentPolicy.isImage(name: url.lastPathComponent)
                let limit = isImage ? ImageAttachmentPolicy.maxFileBytes : TextAttachmentPolicy.maxFileBytes
                let data = try handle.read(upToCount: limit + 1) ?? Data()
                let attachment: PendingAttachment
                if isImage {
                    attachment = try ImageAttachmentPolicy.prepare(name: url.lastPathComponent, data: data, existing: loaded,
                        visionEnabled: settings.visionEnabled && !(settings.mmprojPath ?? "").isEmpty)
                } else {
                    attachment = try TextAttachmentPolicy.prepare(name: url.lastPathComponent, data: data,
                        existingNames: Set(loaded.map(\.name)), currentTotalBytes: totalBytes)
                    totalBytes += attachment.byteCount
                }
                loaded.append(attachment)
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
}
