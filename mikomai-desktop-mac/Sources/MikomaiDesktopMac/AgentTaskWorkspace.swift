import SwiftUI
import MikomaiDesktopCore

struct AgentTaskWorkspace: View {
    let tasks: [NativeAgentTask]
    @Binding var selectedTaskID: String?
    let selectedHistory: String
    let onResumeTask: (NativeAgentTask) -> Void

    private var selectedTask: NativeAgentTask? { tasks.first(where: { $0.id == selectedTaskID }) }

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Text("選択したタスクの記録").font(.system(size: 16, weight: .semibold))
                Spacer()
                if let selectedTask {
                    Button("調査を再開") { onResumeTask(selectedTask) }
                }
            }
            if let selectedTask {
                VStack(alignment: .leading, spacing: 4) {
                    Text("ゴール").font(.system(size: 14)).foregroundStyle(.secondary)
                    Text(selectedTask.goal).font(.system(size: 15, weight: .medium)).textSelection(.enabled)
                    Text("実施状況: \(selectedTask.status) · 更新 \(selectedTask.lastEventAt)")
                        .font(.system(size: 13)).foregroundStyle(.secondary)
                }
                .padding(10).frame(maxWidth: .infinity, alignment: .leading)
                .background(Color(nsColor: .controlBackgroundColor), in: RoundedRectangle(cornerRadius: 8))
            } else {
                Text("左側の一覧からタスクを選ぶと、記録を表示します。")
                    .font(.system(size: 16)).foregroundStyle(.secondary)
            }
            ScrollView {
                Text(selectedHistory.isEmpty ? "選択したタスクの記録はありません" : selectedHistory)
                    .font(.system(size: 13, design: .monospaced))
                    .textSelection(.enabled).frame(maxWidth: .infinity, alignment: .leading)
            }
            .padding(10).background(Color(nsColor: .textBackgroundColor), in: RoundedRectangle(cornerRadius: 8))
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
    }
}
