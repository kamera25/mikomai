import MikomaiDesktopCore

@main struct AgentTaskRerunChecks {
    @MainActor static func main() throws {
        _ = NSApplication.shared
        let root = URL(fileURLWithPath: ProcessInfo.processInfo.environment["MIKOMAI_EXECUTION_CHECK_DIR"]!)
        let settings = root.appendingPathComponent("settings.json")
        try Data(#"{"modelPath":null}"#.utf8).write(to: settings)
        setenv("MIKOMAI_SETTINGS_PATH", settings.path, 1)
        setenv("MIKOMAI_DATA_DIR", root.path, 1)
        setenv("MIKOMAI_GRAPH_DB_PATH", root.appendingPathComponent("graph").path, 1)
        let auditDirectory = root.appendingPathComponent("agent-events")
        try FileManager.default.createDirectory(at: auditDirectory, withIntermediateDirectories: true)
        let taskID = UUID().uuidString.lowercased()
        let originalPrompt = "最初の依頼\nVLAN 100を確認してください"
        let audit = try JSONSerialization.data(withJSONObject: ["events": [
            ["event_type": "task_started", "task_id": taskID, "goal": originalPrompt],
            ["event_type": "goal_set", "goal": "後から変更された目標"]
        ]])
        let auditURL = auditDirectory.appendingPathComponent("\(taskID).json")
        try audit.write(to: auditURL)
        let suite = "mikomai.rerun-check.\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: suite)!
        defer { defaults.removePersistentDomain(forName: suite) }
        let model = DesktopModel(defaults: defaults)
        let originalSession = model.activeSessionID!
        model.sessions[0].messages = [ChatMessage(role: .user, text: "別の会話")]
        let originalMessages = model.sessions[0].messages
        model.pendingAgentTaskIDs[originalSession] = "pending-task"
        model.pendingSavedAgentTaskIDs[originalSession] = "saved-task"
        model.pendingAttachments = [PendingAttachment(name: "unrelated.txt", text: "別の添付")]
        // Keep execution queued so this check never invokes an LLM or a device.
        _ = model.chatResponse.begin(sessionID: originalSession)
        model.selectedAgentTaskID = taskID
        model.workspace = .agentHistory
        let host = NSHostingView(rootView: DesktopWindow(model: model))
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 1400, height: 800),
                              styleMask: [.titled], backing: .buffered, defer: false)
        window.contentView = host
        func pump() {
            RunLoop.main.run(until: Date().addingTimeInterval(0.2))
            host.layoutSubtreeIfNeeded()
        }
        func descendants(_ view: NSView) -> [NSView] { [view] + view.subviews.flatMap(descendants) }
        pump()
        let buttons = descendants(host).compactMap { $0 as? KeyboardActionButton }
        let rerun = buttons.first { $0.accessibilityLabel() == "再実行" }!
        let resume = buttons.first { $0.accessibilityLabel() == "調査を再開" }!
        precondition(rerun.convert(rerun.bounds, to: host).midX < resume.convert(resume.bounds, to: host).midX)
        rerun.performClick(nil)
        pump()
        let newSession = model.activeSessionID!
        precondition(newSession != originalSession && model.workspace == .chat)
        precondition(model.sessions.count == 2 && model.activeSession!.messages.isEmpty)
        precondition(model.sessions.first { $0.id == originalSession }!.messages == originalMessages)
        let submission = model.chatQueue.submissions.first!
        precondition(submission.sessionID == newSession && submission.prompt == originalPrompt)
        precondition(submission.attachments.isEmpty)
        precondition(model.pendingAgentTaskIDs[newSession] == nil && model.pendingSavedAgentTaskIDs[newSession] == nil)
        precondition(model.pendingAgentTaskIDs[originalSession] == "pending-task")
        precondition(model.pendingSavedAgentTaskIDs[originalSession] == "saved-task")
        let savedAudit = try Data(contentsOf: auditURL)
        precondition(savedAudit == audit)
        print("PASS: production rerun button precedes resume and queues the first prompt in a fresh chat without resume state or attachments; original chat and audit preserved")
    }
}
