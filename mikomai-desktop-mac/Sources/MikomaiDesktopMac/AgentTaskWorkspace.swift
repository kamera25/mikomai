import SwiftUI
import MikomaiDesktopCore

struct AgentTaskWorkspace: View {
    let tasks: [NativeAgentTask]
    @Binding var selectedTaskID: String?
    let selectedHistory: String
    let onSelectTask: (NativeAgentTask) -> Void
    let onResumeTask: (NativeAgentTask) -> Void

    private var selectedTask: NativeAgentTask? {
        tasks.first(where: { $0.id == selectedTaskID })
    }

    var body: some View {
        HStack(alignment: .top, spacing: 14) {
            List(selection: $selectedTaskID) {
                ForEach(tasks) { task in
                    VStack(alignment: .leading, spacing: 4) {
                        Text(task.goal).lineLimit(2)
                        Text("\(task.status) · \(task.eventCount)件 · \(task.lastEventAt)")
                            .font(.caption).foregroundStyle(.secondary)
                    }.tag(task.id)
                }
            }
            .frame(minWidth: 250)
            .onChange(of: selectedTaskID) { _ in
                if let selectedTask { onSelectTask(selectedTask) }
            }

            VStack(alignment: .leading, spacing: 10) {
                HStack {
                    Text("選択したタスクの記録").font(.headline)
                    Spacer()
                    if let selectedTask {
                        Button("調査を再開") { onResumeTask(selectedTask) }
                    }
                }
                if let selectedTask {
                    VStack(alignment: .leading, spacing: 4) {
                        Text("ゴール").font(.caption).foregroundStyle(.secondary)
                        Text(selectedTask.goal).font(.system(size: 13, weight: .medium)).textSelection(.enabled)
                        Text("実施状況: \(selectedTask.status) · 更新 \(selectedTask.lastEventAt)")
                            .font(.system(size: 11)).foregroundStyle(.secondary)
                    }
                    .padding(10).frame(maxWidth: .infinity, alignment: .leading)
                    .background(Color(nsColor: .controlBackgroundColor), in: RoundedRectangle(cornerRadius: 8))
                }
                ScrollView {
                    Text(selectedHistory).font(.system(size: 11, design: .monospaced))
                        .textSelection(.enabled).frame(maxWidth: .infinity, alignment: .leading)
                }
                .padding(8).background(Color(nsColor: .textBackgroundColor), in: RoundedRectangle(cornerRadius: 8))
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
    }
}
