import SwiftUI
import AppKit
import Foundation
import Darwin
import Security
import CryptoKit
import MikomaiFFI
import MikomaiDesktopCore
import UniformTypeIdentifiers

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

struct DesktopWindow: View {
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
            if !model.recentToolResults.isEmpty {
                Divider()
                VStack(alignment: .leading, spacing: 7) {
                    Text("取得した状態・DB検索").font(.system(size: 11, weight: .semibold)).foregroundStyle(.secondary)
                    ForEach(model.recentToolResults, id: \.id) { item in
                        VStack(alignment: .leading, spacing: 4) {
                            Label(item.tool, systemImage: item.succeeded ? "checkmark.circle" : "exclamationmark.circle")
                                .font(.system(size: 10, weight: .medium)).foregroundStyle(item.succeeded ? Color.secondary : Color.red)
                            Text(item.output.isEmpty ? "結果は空です" : item.output)
                                .font(.system(size: 10, design: .monospaced)).lineLimit(5).textSelection(.enabled)
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
                Text(model.workspace == .chat ? "ネットワークアシスタント" : "Mikomai-Desktop-Mac")
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
                                MessageRow(message: message, isRunning: model.isWorkingInActiveSession && session.messages.last?.id == message.id, onSelectConfig: { config in
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
                        if model.isWorkingInActiveSession && model.activeSession?.messages.last?.agentProgress == nil {
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
                    Button(action: model.stop) { Image(systemName: "stop.fill")
                            .font(.system(size: 10, weight: .semibold))
                            .foregroundStyle(.white).frame(width: 30, height: 30)
                            .background(Color(red: 0.86, green: 0.08, blue: 0.24), in: Circle()) }
                        .buttonStyle(.plain)
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


