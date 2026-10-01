import MikomaiDesktopCore

private struct ExecutionCheckView: View {
    @ObservedObject var model: DesktopModel
    var body: some View {
        HStack(spacing: 16) {
            ExecutionTerminalView(results: model.executionResultsInActiveSession)
                .frame(width: 460)
            VStack(alignment: .leading, spacing: 12) {
                Text(model.isCancelling ? "停止処理中" : "チャット").font(.headline)
                ForEach(model.queuedSubmissionsInActiveSession) { submission in
                    QueuedSubmissionView(submission: submission) { model.removeQueuedSubmission(submission.id) }
                }
                Spacer()
                ChatComposer(text: $model.draft, isFocused: .constant(true), isEnabled: true,
                             onSubmit: model.send, onEscape: {})
                    .frame(height: 100)
            }.padding(16)
        }
        .padding(16)
        .background(Color(nsColor: .windowBackgroundColor))
    }
}

@main struct ExecutionQueueCheck {
    @MainActor static func main() async throws {
        let root = URL(fileURLWithPath: ProcessInfo.processInfo.environment["MIKOMAI_EXECUTION_CHECK_DIR"]!)
        let settings = root.appendingPathComponent("settings.json")
        try Data(#"{"modelPath":null}"#.utf8).write(to: settings)
        setenv("MIKOMAI_SETTINGS_PATH", settings.path, 1)
        setenv("MIKOMAI_DATA_DIR", root.appendingPathComponent("data").path, 1)
        setenv("MIKOMAI_GRAPH_DB_PATH", root.appendingPathComponent("graph").path, 1)
        let suite = "mikomai.execution-queue-check.\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: suite)!
        defer { defaults.removePersistentDomain(forName: suite) }
        defaults.set(root.appendingPathComponent("empty-documents").path, forKey: "mikomai.desktop.mac.documentsDirectory")
        defaults.set(root.appendingPathComponent("knowledge").path, forKey: "mikomai.desktop.mac.knowledgeDirectory")
        let model = DesktopModel(defaults: defaults)
        let a = model.activeSessionID!
        _ = NSApplication.shared
        let host = NSHostingView(rootView: ExecutionCheckView(model: model))
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 1000, height: 600), styleMask: [.titled], backing: .buffered, defer: false)
        window.contentView = host

        func pump() async throws {
            try await Task.sleep(nanoseconds: 80_000_000)
            host.layoutSubtreeIfNeeded()
        }
        func editor(_ view: NSView) -> ChatComposerTextView? {
            if let editor = view as? ChatComposerTextView { return editor }
            return view.subviews.lazy.compactMap { editor($0) }.first
        }
        func waitForResponse() async throws {
            for _ in 0..<250 {
                try await pump()
                if !model.isWorking && model.chatQueue.submissions.isEmpty { return }
            }
            fatalError("queued response did not finish")
        }

        // Check the actual AppKit editor while the shared engine is occupied.
        let fake = model.chatResponse.begin(sessionID: a)!
        try await pump()
        let input = editor(host)!
        precondition(input.isEditable)
        window.makeFirstResponder(input)
        let enter = NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: [], timestamp: 0, windowNumber: window.windowNumber, context: nil, characters: "\r", charactersIgnoringModifiers: "\r", isARepeat: false, keyCode: 36)!
        input.insertText("生成中の投稿", replacementRange: NSRange(location: NSNotFound, length: 0))
        input.keyDown(with: enter)
        precondition(model.chatQueue.submissions.count == 1 && model.draft.isEmpty)
        model.stop()
        try await pump()
        precondition(input.isEditable && model.isCancelling)
        input.insertText("停止中の投稿", replacementRange: NSRange(location: NSNotFound, length: 0))
        input.keyDown(with: enter)
        precondition(model.chatQueue.submissions.count == 2)
        for submission in model.chatQueue.submissions { model.removeQueuedSubmission(submission.id) }
        model.chatResponse.finish(fake)

        // Exercise production cleanup -> automatic FIFO draining across sessions.
        model.draft = "最初の質問"
        model.send()
        precondition(model.isWorking)
        model.draft = "VLANの仕組みを解説"
        model.pendingAttachments = [PendingAttachment(name: "notes.md", text: "VLAN 10")]
        model.send()
        model.createSession()
        let b = model.activeSessionID!
        model.draft = "hello"
        model.send()
        model.stop()
        model.draft = "こんばんは"
        model.send()
        precondition(model.isCancelling && model.chatQueue.submissions.count == 3)
        model.draft = "未送信の下書き"
        try await waitForResponse()
        let first = model.sessions.first { $0.id == a }!
        let second = model.sessions.first { $0.id == b }!
        precondition(first.messages.filter { $0.role == .user }.map(\.text) == ["最初の質問", "VLANの仕組みを解説"])
        precondition(first.messages.last(where: { $0.role == .user })?.attachments == ["notes.md"])
        precondition(first.messages[1].text.contains("停止しました"))
        // No model is loaded in this harness: the attached request fails, and
        // later greeting requests must still drain successfully.
        precondition(first.messages.last!.text.hasPrefix("エラー:"))
        precondition(second.messages.filter { $0.role == .user }.map(\.text) == ["hello", "こんばんは"])
        precondition(second.messages.last!.role == .assistant && !second.messages.last!.text.isEmpty)
        precondition(model.activeSessionID == b && model.draft == "未送信の下書き")

        // Real Swift callbacks and real OS probes, restricted to loopback.
        model.draft = "ping 127.0.0.1 count 1"
        model.send()
        try await waitForResponse()
        model.draft = "traceroute 127.0.0.1"
        model.send()
        try await waitForResponse()
        try await pump()
        let results = model.executionResultsInActiveSession
        precondition(results.count == 2, "both local probes must reach the execution pane")
        precondition(results[0].tool == "self_network_ping" && results[0].succeeded)
        precondition(results[0].command == "/sbin/ping -c 1 127.0.0.1")
        precondition(results[0].output.contains("1 packets transmitted"))
        precondition(results[1].tool == "self_network_traceroute" && results[1].succeeded)
        precondition(results[1].output.contains("traceroute to"), "stderr header must remain visible")

        let previewRequest = model.chatResponse.begin(sessionID: b)!
        model.chatResponse.cancel()
        model.draft = "この結果をもとに次の調査をお願いします"
        model.send()
        try await pump()
        let bitmap = host.bitmapImageRepForCachingDisplay(in: host.bounds)!
        host.cacheDisplay(in: host.bounds, to: bitmap)
        try bitmap.representation(using: .png, properties: [:])!.write(to: root.appendingPathComponent("execution-queue.png"))
        for submission in model.chatQueue.submissions { model.removeQueuedSubmission(submission.id) }
        model.chatResponse.finish(previewRequest)
        print("PASS: editable input and Enter during generation/cancellation, FIFO cleanup, preserved conversation/attachment/draft, real loopback ping/traceroute callbacks, terminal rendering")
    }
}
