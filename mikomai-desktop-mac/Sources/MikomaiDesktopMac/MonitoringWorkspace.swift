import SwiftUI
import AppKit
import MikomaiDesktopCore

struct MonitoringWorkspace: View {
    @ObservedObject var model: DesktopModel

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack {
                Text("CPU監視").font(.system(size: 24, weight: .semibold))
                Spacer()
                Text(model.watchStatus).font(.system(size: 14)).foregroundStyle(.secondary).lineLimit(2)
                AccessibleButton("更新") { model.refreshWatches() }
            }
            watchContent
        }
        .padding(24)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .onAppear { model.refreshWatches() }
    }

    private var watchContent: some View {
        VStack(alignment: .leading, spacing: 12) {
            GroupBox("新しいCPU監視") {
                VStack(alignment: .leading, spacing: 10) {
                    HStack {
                        TextField("監視名", text: $model.watchName)
                        AccessiblePicker("機器", selection: $model.watchDevice, options: [("", "機器を選択")] + model.connections.map { ($0.name, $0.name) }).frame(width: 220)
                    }
                    HStack {
                        TextField("間隔（秒）", text: $model.watchInterval).frame(width: 130)
                        TextField("CPUしきい値（%）", text: $model.watchThreshold).frame(width: 180)
                        TextField("通知メッセージ", text: $model.watchMessage)
                        if model.watchEditingID != nil { AccessibleButton("取消") { model.watchEditingID = nil } }
                        AccessibleButton(model.watchEditingID == nil ? "監視を作成" : "監視を更新") { model.createCPUWatch() }.accessibleButtonStyle(.prominent)
                    }
                    Text("読み取り専用のCPU状態を定期確認し、しきい値を超えたときに通知します。操作や設定変更は実行しません。")
                        .font(.system(size: 13)).foregroundStyle(.secondary)
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
                            Text("監視設定はありません").font(.system(size: 17, weight: .semibold))
                            Text("機器とCPUしきい値を指定して監視を作成できます。").font(.system(size: 13)).foregroundStyle(.secondary)
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
                        Text(watch.name).font(.system(size: 17, weight: .semibold))
                        Text("\(watch.status == "enabled" ? "有効" : "停止中") · \(watch.ir.schedule.every) · \(watchDeviceName(watch))")
                            .font(.system(size: 13)).foregroundStyle(.secondary)
                    }
                    Spacer()
                    AccessibleButton("今すぐ実行") { model.runWatch(watch) }
                    AccessibleButton("編集") { model.editWatch(watch) }
                    AccessibleButton(watch.status == "enabled" ? "停止" : "再開") { model.setWatch(watch, enabled: watch.status != "enabled") }
                    AccessibleButton("監視を削除: \(watch.name)", role: .destructive) { model.deleteWatch(watch) } label: { Image(systemName: "trash") }
                        .accessibilityLabel("監視を削除: \(watch.name)")
                }
                if let error = watch.lastError { Label(error, systemImage: "exclamationmark.triangle").font(.system(size: 13)).foregroundStyle(.orange) }
                if let latest = watch.history?.last {
                    Text("直近: \(latest.completedAt) · \(latest.error ?? (latest.notifications.isEmpty ? "通知なし" : latest.notifications.map(\.message).joined(separator: "、")))")
                        .font(.system(size: 13)).foregroundStyle(latest.error == nil ? Color.secondary : Color.orange)
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

}
