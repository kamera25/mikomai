import SwiftUI
import AppKit
import Foundation
import Darwin
import MikomaiBindings
import MikomaiDesktopCore
import UniformTypeIdentifiers

// MARK: - Main Desktop Window

private struct WindowAccessor: NSViewRepresentable {
    @Binding var window: NSWindow?

    final class ObserverView: NSView {
        var onWindowChange: ((NSWindow?) -> Void)?

        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            KeyboardNavigation.schedule(in: window)
            onWindowChange?(window)
        }
    }

    func makeNSView(context: Context) -> ObserverView {
        let view = ObserverView()
        view.onWindowChange = { newWindow in
            DispatchQueue.main.async {
                self.window = newWindow
            }
        }
        return view
    }

    func updateNSView(_ nsView: ObserverView, context: Context) {
        nsView.onWindowChange = { newWindow in
            DispatchQueue.main.async {
                self.window = newWindow
            }
        }
        if window != nsView.window && nsView.window != nil {
            DispatchQueue.main.async {
                self.window = nsView.window
            }
        }
    }
}

private struct WindowTitleDragRegion: NSViewRepresentable {
    final class DragView: NSView {
        override var mouseDownCanMoveWindow: Bool { true }

        override func mouseDown(with event: NSEvent) {
            // The custom toolbar title consumes AppKit's titlebar clicks.
            // Handle the second click before starting another drag session.
            if event.clickCount == 2 {
                window?.performZoom(nil)
                return
            }
            window?.performDrag(with: event)
        }
    }

    func makeNSView(context: Context) -> DragView { DragView() }
    func updateNSView(_ nsView: DragView, context: Context) {}
}

private struct InitialHistoryPaneSizing: NSViewRepresentable {
    let width: CGFloat

    final class SizingView: NSView {
        var initialWidth: CGFloat = 248
        private var didSetWidth = false
        private var isScheduled = false

        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            scheduleSizing()
        }

        override func layout() {
            super.layout()
            scheduleSizing()
        }

        func scheduleSizing() {
            guard window != nil, !didSetWidth, !isScheduled else { return }
            isScheduled = true
            DispatchQueue.main.async { [weak self] in
                guard let self else { return }
                self.isScheduled = false
                var ancestor = self.superview
                while let view = ancestor {
                    if let split = view as? NSSplitView, split.isVertical,
                       split.subviews.count > 1, split.bounds.width > 0 {
                        // SwiftUI's idealWidth is only a proposal. Set the initial
                        // divider once, then leave subsequent user resizing alone.
                        self.didSetWidth = true
                        split.setPosition(self.initialWidth, ofDividerAt: 0)
                        return
                    }
                    ancestor = view.superview
                }
            }
        }
    }

    func makeNSView(context: Context) -> SizingView { SizingView() }
    func updateNSView(_ nsView: SizingView, context: Context) {
        nsView.initialWidth = width
        nsView.scheduleSizing()
    }
}

private struct ToolbarHeaderSizing: NSViewRepresentable {
    @Binding var rightPaneWidth: CGFloat
    let showsRightPane: Bool

    final class ObserverView: NSView {
        var onResize: ((NSWindow) -> Void)?

        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            NotificationCenter.default.removeObserver(self)
            if let window {
                NotificationCenter.default.addObserver(self, selector: #selector(resized),
                    name: NSWindow.didResizeNotification, object: window)
                NotificationCenter.default.addObserver(self, selector: #selector(resized),
                    name: NSSplitView.didResizeSubviewsNotification, object: nil)
                resized()
            }
        }

        @objc private func resized() {
            DispatchQueue.main.async { [weak self] in
                guard let self, let window = self.window else { return }
                self.onResize?(window)
            }
        }

        deinit { NotificationCenter.default.removeObserver(self) }
    }

    func makeNSView(context: Context) -> ObserverView { ObserverView() }

    func updateNSView(_ nsView: ObserverView, context: Context) {
        nsView.onResize = { window in
            if showsRightPane, let content = window.contentView,
               let split = Self.mainSplit(in: content), let pane = split.subviews.last {
                let width = pane.frame.width
                if width > 0 && abs(rightPaneWidth - width) > 0.5 { rightPaneWidth = width }
            }
        }
        DispatchQueue.main.async {
            if let window = nsView.window { nsView.onResize?(window) }
        }
    }

    private static func mainSplit(in view: NSView) -> NSSplitView? {
        if let split = view as? NSSplitView, split.isVertical, split.subviews.count > 1 { return split }
        return view.subviews.lazy.compactMap { mainSplit(in: $0) }.first
    }
}

struct DesktopWindow: View {
    @ObservedObject var model: DesktopModel
    @Environment(\.colorScheme) private var colorScheme

    private var chatBackground: Color {
        colorScheme == .dark ? Color(white: 0.2) : Color(nsColor: .windowBackgroundColor)
    }
    @State private var mentionPresentation = ChatMentionPresentation()
    @State private var isChatInputFocused = false
    @State private var isDropTargeted = false
    @StateObject private var diagramPresentation = NetworkDiagramPresentation()
    private var mentionContext: ChatMentionContext? { mentionPresentation.context }
    private var showsHostSuggestions: Bool { mentionPresentation.isVisible(candidateCount: hostSuggestions.count) }
    @State private var hostSuggestionIndex = 0
    @State private var mentionCompletion: ChatMentionCompletion?

    @State private var window: NSWindow?
    @State private var isTiled = false
    @State private var wasHistoryOpenBeforeTiling = true
    @State private var wasRightPaneOpenBeforeTiling = false
    @State private var currentContainerWidth: CGFloat = 1120

    private let historyWidth: CGFloat = 248
    @State private var isHistoryOpen = true
    @AppStorage("mikomai.desktop.mac.rightPaneWidth") private var rightPaneWidth = 330.0
    @State private var isRightPaneOpen = false
    @State private var rightPaneTab = "diff"
    @State private var renderedRightPaneWidth: CGFloat = 330
    @State private var isAtChatBottom = true
    @State private var chatScrollFollow = ChatScrollFollowState()
    @State private var selectedConnectionID: UUID?
    @State private var operationAlert = ""
    @State private var isOperationRunning = false
    @State private var operationRationale = "選択した変更案を適用する"


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
            HSplitView {
                if showsHistorySidebar && isHistoryOpen {
                    historySidebar
                        .frame(minWidth: 180, idealWidth: historyWidth, maxWidth: 420)
                        .background(InitialHistoryPaneSizing(width: historyWidth))
                }
                VStack(spacing: 0) {
                    Group {
                        switch model.workspace {
                        case .chat: selectedHistoryWorkspace
                        case .connections: ConnectionsWorkspace(model: model)
                        case .agentHistory: agentHistoryWorkspace
                        case .monitoring: MonitoringWorkspace(model: model)
                        case .settings: SettingsWorkspace(model: model)
                        }
                    }
                    statusBar
                }
                .frame(minWidth: isTiled ? 0 : 440, maxWidth: .infinity)
                .frame(width: isTiled && !isHistoryOpen && !isRightPaneOpen
                       ? max(0, geometry.size.width - 50) : nil)
                .background(Color(nsColor: .windowBackgroundColor))
                if model.workspace == .chat && isRightPaneOpen {
                    rightSidePane
                        .frame(minWidth: 180, idealWidth: rightPaneWidth, maxWidth: 600)
                }
            }
            .frame(width: max(0, geometry.size.width - 50))
        }
        .background(Color(nsColor: .underPageBackgroundColor))
        .background(WindowAccessor(window: $window))
        .background(KeyboardNavigationScope())
        .onAppear {
            currentContainerWidth = geometry.size.width
            evaluateTiling(containerWidth: geometry.size.width)
        }
        .onChange(of: geometry.size.width) { newWidth in
            currentContainerWidth = newWidth
            evaluateTiling(containerWidth: newWidth)
        }
        }
        .toolbar { windowToolbar }
        .toolbarBackground(Color(nsColor: .controlBackgroundColor), for: .windowToolbar)
        .toolbarBackground(.visible, for: .windowToolbar)
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
            if rightPaneTab != "debug" {
                rightPaneTab = "diff"
                isRightPaneOpen = true
            }
        }
        .onChange(of: model.executionResultsInActiveSession.last?.id) { id in
            guard id != nil, !(isRightPaneOpen && rightPaneTab == "debug") else { return }
            rightPaneTab = "execution"
            isRightPaneOpen = true
        }
        .onChange(of: model.workspace) { workspace in
            if (workspace == .chat || workspace == .agentHistory) && !isTiled {
                isHistoryOpen = true
            }
        }
        .onAppear { model.startWatchService() }
        .onReceive(NotificationCenter.default.publisher(for: NSApplication.willTerminateNotification)) { _ in model.stopWatchService() }
        .onReceive(NotificationCenter.default.publisher(for: NSWindow.didResizeNotification)) { notif in
            if let w = notif.object as? NSWindow, window == nil || w == window {
                if window == nil { window = w }
                evaluateTiling(containerWidth: currentContainerWidth)
            }
        }
        .onReceive(NotificationCenter.default.publisher(for: NSWindow.didMoveNotification)) { notif in
            if let w = notif.object as? NSWindow, window == nil || w == window {
                if window == nil { window = w }
                evaluateTiling(containerWidth: currentContainerWidth)
            }
        }
        .alert("保存エラー", isPresented: Binding(
            get: { !model.persistenceError.isEmpty },
            set: { if !$0 { model.persistenceError = "" } }
        )) { Button("OK", role: .cancel) {} } message: { Text(model.persistenceError) }
        .alert(item: $model.watchAlert) { alert in
            Alert(title: Text("ネットワーク監視"), message: Text(alert.message), dismissButton: .default(Text("閉じる")))
        }
    }

    private func evaluateTiling(containerWidth: CGFloat) {
        let targetWindow = window ?? NSApp.keyWindow ?? NSApp.mainWindow ?? NSApp.windows.first(where: { $0.isVisible && $0.canBecomeKey })
        let windowFrame = targetWindow?.frame
        let screenFrame = targetWindow?.screen?.visibleFrame
        let newIsTiled = PaneResizePolicy.shouldCollapsePanesForTiling(
            containerWidth: Double(containerWidth),
            windowFrame: windowFrame,
            screenVisibleFrame: screenFrame
        )
        guard newIsTiled != isTiled else { return }
        isTiled = newIsTiled
        if newIsTiled {
            wasHistoryOpenBeforeTiling = isHistoryOpen
            wasRightPaneOpenBeforeTiling = isRightPaneOpen
            withAnimation(.easeInOut(duration: 0.18)) {
                isHistoryOpen = false
                isRightPaneOpen = false
            }
        } else {
            withAnimation(.easeInOut(duration: 0.18)) {
                isHistoryOpen = wasHistoryOpenBeforeTiling
                isRightPaneOpen = wasRightPaneOpenBeforeTiling
            }
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
        AccessibleButton(item.rawValue, value: model.workspace == item ? "選択中" : "未選択") {
            model.workspace = item
            if (item == .chat || item == .agentHistory) && !isTiled { isHistoryOpen = true }
        } label: {
            Image(systemName: item.icon).font(.system(size: 16, weight: .medium))
                .foregroundStyle(model.workspace == item ? .primary : .secondary)
                .frame(width: 34, height: 34)
                .background(model.workspace == item ? Color(nsColor: .selectedContentBackgroundColor).opacity(0.16) : .clear, in: RoundedRectangle(cornerRadius: 6))
                .contentShape(Rectangle())
        }
        .accessibleButtonStyle(.plain)
        .accessibleButtonHoverHighlight(cornerRadius: 6)
        .help(item.rawValue)
    }

    private var historySidebar: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack {
                if model.workspace == .agentHistory {
                    Text("エージェント履歴").font(.system(size: 14, weight: .semibold))
                } else {
                    AccessibleButton("新規チャット") { model.createSession() } label: {
                        HStack(spacing: 6) {
                            Text("新規チャット")
                            Image(systemName: "square.and.pencil")
                        }
                        .frame(minHeight: 36)
                        .padding(.horizontal, 6)
                            .contentShape(Rectangle())
                    }
                    .accessibleButtonStyle(.plain)
                    .accessibleButtonHoverHighlight(cornerRadius: 6)
                    .help("新規チャット")
                    .accessibilityLabel("新規チャット")
                }
                Spacer()
                if model.workspace == .agentHistory {
                    AccessibleButton("エージェント履歴を更新") { model.refreshAgentTasks() } label: {
                        Image(systemName: "arrow.clockwise")
                            .frame(width: 28, height: 28)
                            .contentShape(Rectangle())
                    }
                    .accessibleButtonStyle(.plain)
                    .accessibleButtonHoverHighlight(cornerRadius: 6)
                    .help("エージェント履歴を更新")
                    .accessibilityLabel("エージェント履歴を更新")
                }
            }
            .padding(.horizontal, 14).padding(.vertical, 12)
            Divider()
            ScrollView {
                VStack(alignment: .leading, spacing: 2) {
                    if model.workspace == .agentHistory {
                        AgentTaskHistoryList(
                            tasks: model.agentTasks,
                            selectedTaskID: $model.selectedAgentTaskID,
                            onSelect: { task in model.loadAgentTaskHistory(task) },
                            onDelete: { task in model.deleteAgentTask(task) }
                        )
                    } else {
                        ForEach(model.sessions) { session in
                            SessionRow(session: session, isSelected: session.id == model.activeSessionID,
                                       onSelect: { model.select(session.id) },
                                       onRename: { model.renameSession(session.id, title: $0) },
                                       onDelete: { model.deleteSession(session.id) })
                        }
                    }
                }.padding(8)
            }
            if model.workspace == .chat && !model.recentToolResults.isEmpty {
                Divider()
                VStack(alignment: .leading, spacing: 7) {
                    Text("取得した状態・DB検索").font(.system(size: 13, weight: .semibold)).foregroundStyle(.secondary)
                    ForEach(model.recentToolResults, id: \.id) { item in
                        VStack(alignment: .leading, spacing: 4) {
                            Label(item.tool, systemImage: item.succeeded ? "checkmark.circle" : "exclamationmark.circle")
                                .font(.system(size: 12, weight: .medium)).foregroundStyle(item.succeeded ? Color.secondary : Color.red)
                            Text(item.output.isEmpty ? "結果は空です" : item.output)
                                .font(.system(size: 12, design: .monospaced)).lineLimit(5).textSelection(.enabled)
                        }
                        .padding(7).frame(maxWidth: .infinity, alignment: .leading)
                        .background(Color(nsColor: .textBackgroundColor), in: RoundedRectangle(cornerRadius: 6))
                    }
                }.padding(10)
            }
            Spacer(minLength: 0)
            Divider()
            HStack(spacing: 8) {
                Image(systemName: "books.vertical").foregroundStyle(.secondary)
                VStack(alignment: .leading, spacing: 2) {
                    Text("ローカルナレッジ").font(.system(size: 13, weight: .medium))
                    Text(URL(fileURLWithPath: model.documentsDirectory).lastPathComponent).font(.system(size: 12, design: .monospaced)).foregroundStyle(.secondary)
                }
            }.padding(12)
        }
        .background(Color(nsColor: .controlBackgroundColor).opacity(0.7))
        .accessibleButtonHoverHighlight(cornerRadius: 6)
    }

    private var showsHistorySidebar: Bool {
        model.workspace == .chat || model.workspace == .agentHistory
    }

    private var agentHistoryWorkspace: some View {
        AgentTaskWorkspace(
            tasks: model.agentTasks,
            selectedTaskID: $model.selectedAgentTaskID,
            selectedHistory: model.selectedTaskHistory,
            onRefresh: { model.refreshAgentTasks() },
            onRerunTask: { task in
                model.rerunAgentTask(task)
            },
            canRerunTask: !model.isLoadingModel,
            onResumeTask: { task in
                model.workspace = .chat
                model.resumeAgentTask(task)
            },
            onDeleteTask: { task in model.deleteAgentTask(task) },
            onDeleteAllTasks: { model.deleteAllAgentTasks() }
        )
        .padding(20)
        .onAppear { model.refreshAgentTasks() }
    }

    @ViewBuilder
    private var selectedHistoryWorkspace: some View {
        chatWorkspace
    }

    @ToolbarContentBuilder
    private var windowToolbar: some ToolbarContent {
        if #available(macOS 26.0, *) {
            ToolbarItem(id: "windowHeader", placement: .principal) { header }
                .sharedBackgroundVisibility(.hidden)
        } else {
            ToolbarItem(id: "windowHeader", placement: .principal) { header }
        }
    }

    private var header: some View {
        HStack(spacing: 10) {
            if showsHistorySidebar {
                paneToggleButton(symbol: "sidebar.left",
                                 title: isHistoryOpen ? "履歴一覧を非表示" : "履歴一覧を表示") {
                    withAnimation(.easeInOut(duration: 0.18)) { isHistoryOpen.toggle() }
                }
            }
            toolbarTitle.overlay(WindowTitleDragRegion())
            if model.workspace == .chat {
                paneToggleButton(symbol: "sidebar.right",
                                 title: isRightPaneOpen ? "作業タブを非表示" : "作業タブを表示") {
                    withAnimation(.easeInOut(duration: 0.18)) { isRightPaneOpen.toggle() }
                }
                if isRightPaneOpen {
                    RightPaneTabHeader(selectedTab: rightPaneTab, onSelect: { rightPaneTab = $0 })
                        .frame(width: max(154, renderedRightPaneWidth - 20))
                        .overlay(alignment: .leading) { Divider() }
                }
            }
        }
        // The native toolbar reserves the area occupied by the window controls.
        .frame(minWidth: 0, idealWidth: max(0, currentContainerWidth - 120),
               maxWidth: max(0, currentContainerWidth - 120))
        .frame(height: 36)
        .background(ToolbarHeaderSizing(rightPaneWidth: $renderedRightPaneWidth,
                                        showsRightPane: model.workspace == .chat && isRightPaneOpen))
    }

    private func paneToggleButton(symbol: String, title: String,
                                  action: @escaping () -> Void) -> some View {
        AccessibleButton(title, action: action) {
            Image(systemName: symbol)
                .frame(width: 28, height: 28)
                .contentShape(Rectangle())
        }
        .accessibleButtonStyle(.plain)
        .accessibleButtonHoverHighlight(cornerRadius: 6)
        .help(title)
        .accessibilityLabel(title)
    }

    private var toolbarTitle: some View {
        Text(headerTitle)
            .font(.system(size: 16, weight: .semibold))
            .lineLimit(1)
            .truncationMode(.tail)
            .frame(maxWidth: .infinity, alignment: .leading)
    }

    private var headerTitle: String {
        guard model.workspace == .chat else { return model.workspace.rawValue }
        return model.activeSession?.title ?? "mikomai"
    }

    private func handleSelectedConfig(_ config: String) {
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
    }

    private func messageRow(_ message: ChatMessage, in session: ChatSession) -> some View {
        MessageRow(
            message: message,
            isRunning: model.isWorkingInActiveSession && session.messages.last?.id == message.id,
            onSelectConfig: handleSelectedConfig,
            onShowTraceResults: {
                model.selectedExecutionMessageID = message.id
                model.workspace = .chat
                rightPaneTab = "execution"
                isRightPaneOpen = true
            }
        )
    }

    private var rightSidePane: some View {
        VStack(spacing: 0) {
            if rightPaneTab == "diff" {
                operationDiffPane
            } else if rightPaneTab == "debug" {
                CoreDebugView(model: model)
            } else if rightPaneTab == "execution" {
                ExecutionTerminalView(results: model.displayedExecutionResults)
            } else {
                VStack(alignment: .leading, spacing: 10) {
                    Label("投入ログ", systemImage: "text.alignleft").font(.system(size: 14, weight: .semibold))
                    if model.operationLogs.isEmpty {
                        Text("変更案の確認と投入を行うと、各手順の結果がここに表示されます。")
                            .font(.system(size: 13)).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
                    } else {
                        ScrollView {
                            VStack(alignment: .leading, spacing: 5) {
                                ForEach(Array(model.operationLogs.enumerated()), id: \.offset) { _, line in
                                    Text(line).font(.system(size: 12, design: .monospaced)).textSelection(.enabled)
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
        .accessibleButtonHoverHighlight(cornerRadius: 6)
    }

    private var operationDiffPane: some View {
        VStack(alignment: .leading, spacing: 10) {
            Label("変更計画", systemImage: "doc.text.magnifyingglass")
                .font(.system(size: 14, weight: .semibold))
            if model.operationProposal.isEmpty {
                Text("回答の設定コマンドを右クリックし、「変更計画として確認」を選ぶと、ここで現状との差分を確認できます。")
                    .font(.system(size: 13)).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
                Spacer()
            } else {
                AccessiblePicker("対象機器", selection: $selectedConnectionID, options: [(Optional<UUID>.none, "機器を選択")] + model.connections.filter { ["ssh", "telnet"].contains(($0.connectionType ?? "SSH").lowercased()) }.map { (Optional($0.id), "\($0.name) (\($0.host))") })
                .disabled(model.operationPlan != nil || isOperationRunning)
                ChatComposer(text: $model.operationProposal, isFocused: .constant(false),
                             isEnabled: model.operationPlan == nil && !isOperationRunning, onSubmit: {}, onEscape: {},
                             submitsOnReturn: false, accessibilityLabel: "変更案の設定コマンド",
                             editorFont: .monospacedSystemFont(ofSize: 12, weight: .regular),
                             placeholder: "設定コマンドを入力…")
                    .frame(minHeight: 95, maxHeight: 170)
                    .disabled(model.operationPlan != nil || isOperationRunning)
                AccessibleDisclosureGroup("取得した現状のConfig") {
                    ScrollView {
                        Text(model.operationBeforeConfig.isEmpty ? "まだ取得していません。" : model.operationBeforeConfig)
                            .font(.system(size: 11, design: .monospaced)).textSelection(.enabled)
                            .frame(maxWidth: .infinity, alignment: .leading)
                    }.frame(maxHeight: 110)
                }
                TextField("変更の理由", text: $operationRationale)
                    .textFieldStyle(.roundedBorder).font(.system(size: 13))
                    .disabled(model.operationPlan != nil || isOperationRunning)
                if !model.operationBeforeConfig.isEmpty {
                    Text(model.operationAfterConfig.isEmpty ? "提案コマンド" : "投入後の実機差分").font(.system(size: 13, weight: .semibold))
                    ScrollView {
                        VStack(alignment: .leading, spacing: 2) {
                            ForEach(Array(operationPreviewLines.enumerated()), id: \.offset) { _, item in
                                Text(item).font(.system(size: 12, design: .monospaced))
                                    .foregroundStyle(item.hasPrefix("+") ? Color.green : Color.secondary)
                                    .frame(maxWidth: .infinity, alignment: .leading)
                            }
                        }
                    }.frame(maxHeight: 180)
                }
                if let plan = model.operationPlan {
                    Text("状態: \(operationStatusLabel(plan.status))")
                        .font(.system(size: 13, weight: .medium)).foregroundStyle(.secondary)
                    AccessibleDisclosureGroup("計画の照合情報") {
                        Text("ID: \(plan.id)\nSHA-256: \(plan.planHash)")
                            .font(.system(size: 11, design: .monospaced)).textSelection(.enabled)
                            .foregroundStyle(.secondary).lineLimit(4)
                    }
                }
                if !operationAlert.isEmpty {
                    Text(operationAlert).font(.system(size: 12)).foregroundStyle(.red).fixedSize(horizontal: false, vertical: true)
                }
                Spacer(minLength: 0)
                if isOperationRunning {
                    HStack(spacing: 7) { ProgressView().controlSize(.small); Text(model.operationPhase).font(.system(size: 13)) }
                    if model.operationWaitingDecision, let id=model.operationTaskID {
                        HStack {
                            AccessibleButton("待機を続ける") { try? MikomaiService().resume(taskId:id) }
                            AccessibleButton("中止") { try? MikomaiService().cancel(taskId:id) }
                        }
                    }
                } else if model.operationPlan == nil {
                    AccessibleButton("現状を取得して差分を確認") { Task { await prepareOperationPlan() } }
                        .accessibleButtonStyle(.prominent).disabled(selectedConnectionID == nil || model.connections.isEmpty)
                } else if model.operationPlan?.status == "pending" {
                    AccessibleButton("確認して承認・投入") { Task { await approveAndExecutePlan() } }
                        .accessibleButtonStyle(.prominent)
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
        guard !isOperationRunning, let id = selectedConnectionID else { return }
        isOperationRunning = true
        operationAlert = ""
        if let error = await model.operationCoordinator.preparePlan(connectionID: id, rationale: operationRationale) {
            operationAlert = error
        }
        isOperationRunning = false
    }

    private func approveAndExecutePlan() async {
        guard !isOperationRunning else { return }
        isOperationRunning = true
        operationAlert = ""
        rightPaneTab = "logs"
        if let error = await model.operationCoordinator.approveAndExecutePlan() {
            operationAlert = error
        } else if model.operationPlan?.status == "executed" || model.operationPhase.contains("差分を確認") {
            rightPaneTab = "diff"
        }
        isOperationRunning = false
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


    private var statusBar: some View {
        HStack(spacing: 14) {
            HStack(spacing: 5) {
                Circle()
                    .fill(model.modelStatus.hasPrefix("読み込み済み") || model.modelStatus.hasPrefix("利用可能") ? Color.green : Color.orange)
                    .frame(width: 7, height: 7)
                Text(model.modelStatus)
                    .font(.system(size: 13))
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
            }

            Divider().frame(height: 12)

            HStack(spacing: 4) {
                Image(systemName: "books.vertical").font(.system(size: 10)).foregroundStyle(.secondary)
                Text(URL(fileURLWithPath: model.documentsDirectory).lastPathComponent)
                    .font(.system(size: 12, design: .monospaced))
                    .foregroundStyle(.secondary)
            }

            Spacer()

            if !isTiled {
                HStack(spacing: 4) {
                    Image(systemName: "point.3.connected.trianglepath.dotted").font(.system(size: 10)).foregroundStyle(.secondary)
                    Text("登録機器: \(model.connections.count)台")
                        .font(.system(size: 13))
                        .foregroundStyle(.secondary)
                }

                if model.workspace == .chat, let count = model.activeSession?.messages.count {
                    Divider().frame(height: 12)
                    Text("メッセージ: \(count)件")
                        .font(.system(size: 13))
                        .foregroundStyle(.secondary)
                }
            }
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 6)
        .background(Color(nsColor: .controlBackgroundColor))
        .overlay(alignment: .top) { Divider() }
    }

    private var chatWorkspace: some View {
        VStack(spacing: 0) {
            GeometryReader { geometry in
                ScrollViewReader { proxy in
                    ScrollView {
                    VStack(alignment: .leading, spacing: 18) {
                        if model.activeSession?.messages.isEmpty ?? true { emptyState }
                        if let session = model.activeSession {
                            ForEach(session.messages) { message in
                                messageRow(message, in: session).id(message.id)
                            }
                        }
                        if model.isWorkingInActiveSession && model.activeSession?.messages.last?.agentProgress == nil {
                            WorkingStatusView(isCancelling: model.isCancelling)
                            if model.chatWaitingDeviceDecision, let id=model.activeRustTaskID {
                                AccessibleButton("機器のロック待ちを続ける") { try? MikomaiService().resume(taskId:id) }
                            }
                        }
                        ForEach(model.queuedSubmissionsInActiveSession) { submission in
                            QueuedSubmissionView(submission: submission) {
                                model.removeQueuedSubmission(submission.id)
                            }
                        }
                    }
                    .frame(width: min(760, max(0, geometry.size.width - 48)))
                    .frame(maxWidth: .infinity).padding(.horizontal, 24).padding(.vertical, 24)
                    .id("chatBottom")
                    .background(ChatScrollObserver { top, atBottom in
                        chatScrollFollow.observe(contentTop: top, isAtBottom: atBottom)
                        isAtChatBottom = atBottom
                        chatScrollFollow.updateViewport(isAtBottom: atBottom)
                    })
                    }
                    .onChange(of: model.activeSession?.messages.last?.text ?? "") { _ in
                        if chatScrollFollow.followsOutput { proxy.scrollTo("chatBottom", anchor: .bottom) }
                    }
                    .onChange(of: model.activeSession?.messages.count ?? 0) { _ in
                        if chatScrollFollow.followsOutput { proxy.scrollTo("chatBottom", anchor: .bottom) }
                    }
                    .onChange(of: model.queuedSubmissionsInActiveSession.last?.id) { _ in
                        if chatScrollFollow.followsOutput { proxy.scrollTo("chatBottom", anchor: .bottom) }
                    }
                    .onChange(of: model.activeSessionID) { _ in
                        isAtChatBottom = true
                        chatScrollFollow.resetForSessionChange()
                        proxy.scrollTo("chatBottom", anchor: .bottom)
                    }
                    .overlay(alignment: .bottom) {
                        if !isAtChatBottom {
                            AccessibleButton("最新のメッセージへ移動") {
                                chatScrollFollow.resume()
                                proxy.scrollTo("chatBottom", anchor: .bottom)
                            } label: {
                                Label("一番下に移動", systemImage: "arrow.down")
                                    .font(.system(size: 14, weight: .medium))
                                    .padding(.horizontal, 14).padding(.vertical, 8)
                                    .background(Color(nsColor: .controlBackgroundColor), in: RoundedRectangle(cornerRadius: 8))
                                    .overlay(RoundedRectangle(cornerRadius: 8).stroke(Color(nsColor: .separatorColor), lineWidth: 0.7))
                            }
                            .accessibilityIdentifier("chat-scroll-to-bottom")
                            .accessibilityLabel("最新のメッセージへ移動")
                            .accessibleButtonStyle(.plain).padding(.bottom, 8)
                        }
                    }
                }
            }
            composer
        }
        .background(chatBackground)
        .dropDestination(for: URL.self) { urls, _ in
            guard !urls.isEmpty else { return false }
            return model.attachFiles(at: urls)
        } isTargeted: { targeted in
            withAnimation(.easeInOut(duration: 0.15)) {
                isDropTargeted = targeted
            }
        }
        .overlay {
            if isDropTargeted {
                dropOverlay
            }
        }
        .modifier(ChatDiagramExpansion(presentation: diagramPresentation))
        .accessibleButtonHoverHighlight(cornerRadius: 6)
        .onChange(of: model.activeSessionID) { _ in diagramPresentation.close() }
        .onDisappear { diagramPresentation.close() }
    }

    private var dropOverlay: some View {
        ZStack {
            Color.accentColor.opacity(0.08)
            RoundedRectangle(cornerRadius: 12)
                .strokeBorder(Color.accentColor, style: StrokeStyle(lineWidth: 2, dash: [8, 4]))
                .padding(16)
            VStack(spacing: 12) {
                Image(systemName: "arrow.down.doc.fill")
                    .font(.system(size: 38))
                    .foregroundStyle(Color.accentColor)
                Text("ファイルをドロップして添付")
                    .font(.system(size: 16, weight: .semibold))
                    .foregroundStyle(.primary)
                Text("テキスト (.txt, .md, .json, .yaml, .xml, .log) または画像 (.png, .jpg)")
                    .font(.system(size: 13, weight: .medium))
                    .foregroundStyle(.primary)
                    .multilineTextAlignment(.center)
            }
            .padding(24)
            .background(Color(nsColor: .windowBackgroundColor), in: RoundedRectangle(cornerRadius: 12))
            .shadow(color: Color.black.opacity(0.12), radius: 8, x: 0, y: 4)
        }
        .allowsHitTesting(false)
        .transition(.opacity)
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
                    .font(.system(size: 50))
                    .foregroundStyle(Color.accentColor)
                    .accessibilityHidden(true)
            }
            Text("インフラについて何を行いますか？")
                .font(.system(size: 23, weight: .semibold))
                .multilineTextAlignment(.center)
            LazyVGrid(columns: [GridItem(.adaptive(minimum: 220, maximum: 380))], spacing: 12) {
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
        AccessibleButton(title, fillsWidth: true) {
            mentionPresentation.dismiss()
            model.draft = prompt
            model.send()
        } label: {
            VStack(alignment: .leading, spacing: 12) {
                Image(systemName: icon)
                    .font(.system(size: 20))
                    .foregroundStyle(Color.accentColor)
                Text(title)
                    .font(.system(size: 15, weight: .medium))
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
        .accessibleButtonStyle(.plain)
        .help("クリックして実行")
        .accessibilityLabel(title)
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
                        HistorySelectionRow("\(suggestion.hostname)、\(suggestion.ip)", isSelected: index == hostSuggestionIndex, action: {
                            selectHostSuggestion(suggestion)
                        }) {
                            HStack(spacing: 8) {
                                Image(systemName: icon).font(.system(size: 11)).foregroundStyle(.secondary)
                                    .accessibilityHidden(true)
                                Text(suggestion.hostname).font(.system(size: 14, weight: .medium))
                                Text(suggestion.ip).font(.system(size: 13)).foregroundStyle(.secondary)
                                Spacer(minLength: 0)
                            }
                        }
                        .accessibilityLabel("\(suggestion.hostname)、\(suggestion.ip)")
                        .id(index)
                        .onHover { hovering in if hovering { hostSuggestionIndex = index } }
                    }
                }
                }
                .frame(height: min(180, CGFloat(hostSuggestions.count) * 44))
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
                                AccessibleButton("添付を削除: \(attachment.name)") { model.removeAttachment(attachment.id) } label: {
                                    Image(systemName: "xmark").font(.system(size: 9, weight: .semibold))
                                }.accessibleButtonStyle(.plain).help("添付を削除").accessibilityLabel("添付を削除: \(attachment.name)")
                            }
                            .font(.system(size: 13)).padding(.horizontal, 8).padding(.vertical, 5)
                            .background(Color(nsColor: .controlBackgroundColor), in: RoundedRectangle(cornerRadius: 5))
                        }
                    }
                }.scrollIndicators(.hidden)
            }
            if !model.attachmentError.isEmpty {
                Text(model.attachmentError).font(.system(size: 13)).foregroundStyle(.red)
            }
            HStack(alignment: .bottom, spacing: 10) {
                AccessibleButton("ファイルを添付", action: model.selectAttachments) {
                    Image(systemName: "paperclip")
                        .font(.system(size: 15))
                        .foregroundStyle(.secondary)
                        .frame(width: 28, height: 28)
                        .contentShape(Rectangle())
                }
                .accessibleButtonStyle(.plain).help("テキスト・PNG/JPEG画像を添付").accessibilityLabel("ファイルを添付")

                ChatComposer(text: $model.draft, isFocused: $isChatInputFocused,
                             isEnabled: true, onSubmit: model.send,
                             onEscape: { mentionPresentation.dismiss() },
                             onSuggestionKey: handleSuggestionKey,
                             onMentionContextChanged: { context in
                                 guard mentionContext != context else { return }
                                 hostSuggestionIndex = 0
                                 mentionPresentation.update(context: context)
                             }, completion: mentionCompletion,
                             onFileDrop: { urls in model.attachFiles(at: urls) },
                             onDragTargetChanged: { targeted in
                                 withAnimation(.easeInOut(duration: 0.15)) {
                                     isDropTargeted = targeted
                                 }
                             })
                    .padding(.horizontal, 4)
                    .padding(.vertical, 4)

                if model.isWorking && !ChatSubmissionPolicy.hasContent(prompt: model.draft, attachmentCount: model.pendingAttachments.count) {
                    AccessibleButton(model.isCancelling ? "停止処理中" : "生成を停止", action: model.stop) { Image(systemName: "stop.fill")
                            .font(.system(size: 12, weight: .semibold))
                            .foregroundStyle(.white).frame(width: 30, height: 30)
                            .background(Color(red: 0.86, green: 0.08, blue: 0.24), in: Circle()) }
                        .accessibleButtonStyle(.plain)
                        .disabled(!ChatSubmissionPolicy.canStop(isWorking: model.isWorking, isCancelling: model.isCancelling))
                        .help(model.isCancelling ? "停止処理中" : "生成を停止")
                        .accessibilityLabel(model.isCancelling ? "停止処理中" : "生成を停止")
                } else {
                    AccessibleButton(model.isWorking ? "次回送信予定に追加" : "送信", action: model.send) {
                        Image(systemName: ChatSubmissionPolicy.hasContent(prompt: model.draft, attachmentCount: model.pendingAttachments.count) ? "paperplane.fill" : "arrow.up")
                            .font(.system(size: 14, weight: .semibold))
                            .foregroundStyle(.white).frame(width: 30, height: 30)
                            .background(ChatSubmissionPolicy.hasContent(prompt: model.draft, attachmentCount: model.pendingAttachments.count) ? Color.accentColor : Color.gray.opacity(0.55), in: Circle())
                    }
                        .accessibleButtonStyle(.plain)
                        .disabled(model.isLoadingModel || !ChatSubmissionPolicy.hasContent(prompt: model.draft, attachmentCount: model.pendingAttachments.count))
                        .help(model.isWorking ? "次回送信予定に追加 (Enter)" : "送信 (Enter、Shift+Enter で改行)")
                        .accessibilityLabel(model.isWorking ? "次回送信予定に追加" : "送信")
                }
            }
        }
        .padding(10).background(Color(nsColor: .textBackgroundColor)).clipShape(RoundedRectangle(cornerRadius: 7))
        .overlay(RoundedRectangle(cornerRadius: 7).stroke(isDropTargeted ? Color.accentColor : Color(nsColor: .separatorColor), lineWidth: isDropTargeted ? 1.5 : 0.7))
        .frame(maxWidth: 760).padding(.horizontal, 22).padding(.top, 10).padding(.bottom, 14)
        .frame(maxWidth: .infinity).background(chatBackground)
        .onAppear {
            isChatInputFocused = true
        }
        .onChange(of: model.isWorking) { isWorking in
            if !isWorking { isChatInputFocused = true }
        }

        .onChange(of: hostSuggestions.map(\.hostname)) { _ in
            hostSuggestionIndex = min(hostSuggestionIndex, max(0, hostSuggestions.count - 1))
        }
    }


}
