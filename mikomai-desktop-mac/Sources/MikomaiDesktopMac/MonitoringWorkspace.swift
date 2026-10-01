import SwiftUI
import AppKit
import MikomaiDesktopCore

struct MonitoringWorkspace: View {
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
        AgentTaskWorkspace(
            tasks: model.agentTasks,
            selectedTaskID: $model.selectedAgentTaskID,
            selectedHistory: model.selectedTaskHistory,
            onSelectTask: model.loadAgentTaskHistory,
            onResumeTask: model.resumeAgentTask
        )
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

