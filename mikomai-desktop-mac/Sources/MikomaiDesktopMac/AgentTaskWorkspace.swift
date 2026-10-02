import SwiftUI
import MikomaiDesktopCore

struct HistorySelectionRow<Content: View>: View {
    let isSelected: Bool
    let action: () -> Void
    private let content: Content

    init(isSelected: Bool, action: @escaping () -> Void, @ViewBuilder content: () -> Content) {
        self.isSelected = isSelected
        self.action = action
        self.content = content()
    }

    var body: some View {
        Button(action: action) {
            content
                .frame(maxWidth: .infinity, minHeight: 36, alignment: .leading)
                .padding(.horizontal, 8)
                .padding(.vertical, 4)
                .background(isSelected ? Color(nsColor: .selectedContentBackgroundColor).opacity(0.16) : .clear, in: RoundedRectangle(cornerRadius: 5))
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .frame(maxWidth: .infinity)
        .contentShape(Rectangle())
    }
}

struct AgentTaskHistoryList: View {
    let tasks: [NativeAgentTask]
    @Binding var selectedTaskID: String?
    let onSelect: (NativeAgentTask) -> Void

    var body: some View {
        if tasks.isEmpty {
            Text("エージェント履歴はありません")
                .font(.system(size: 14)).foregroundStyle(.secondary).padding(8)
        } else {
            ForEach(AgentTaskHistoryPresentation.taskDateGroups(tasks)) { group in
                VStack(alignment: .leading, spacing: 2) {
                    historyDayDivider(group.date.map { AgentTaskHistoryPresentation.dateLabel($0) } ?? "日時不明")
                    ForEach(group.tasks) { task in
                        HistorySelectionRow(isSelected: task.id == selectedTaskID, action: {
                            selectedTaskID = task.id
                            onSelect(task)
                        }) {
                            VStack(alignment: .leading, spacing: 4) {
                                Text(task.goal).font(.system(size: 15)).lineLimit(2)
                                HStack(spacing: 8) {
                                    AgentTaskStatusLabel(status: task.status, updatedAt: task.lastEventAt, showsUnknownTime: true)
                                    Text("\(task.eventCount)件")
                                            .font(.system(size: 13)).foregroundStyle(.secondary)
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    private func historyDayDivider(_ title: String) -> some View {
        HStack(spacing: 7) {
            Rectangle().fill(Color.secondary.opacity(0.35)).frame(height: 1)
            Text(title)
                .font(.system(size: 11, weight: .medium)).foregroundStyle(.secondary)
                .fixedSize()
            Rectangle().fill(Color.secondary.opacity(0.35)).frame(height: 1)
        }
        .padding(.horizontal, 6)
        .padding(.top, 10)
        .padding(.bottom, 4)
        .accessibilityElement(children: .combine)
    }
}

struct AgentTaskWorkspace: View {
    let tasks: [NativeAgentTask]
    @Binding var selectedTaskID: String?
    let selectedHistory: [AgentTaskHistoryItem]
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
                    AgentTaskStatusLabel(status: selectedTask.status, updatedAt: selectedTask.lastEventAt, showsStatusName: true)
                }
                .padding(10).frame(maxWidth: .infinity, alignment: .leading)
                .background(Color(nsColor: .controlBackgroundColor), in: RoundedRectangle(cornerRadius: 8))
            } else {
                Text("左側の一覧からタスクを選ぶと、記録を表示します。")
                    .font(.system(size: 16)).foregroundStyle(.secondary)
            }
            ScrollView {
                if selectedHistory.isEmpty {
                    Text("選択したタスクの記録はありません")
                        .font(.system(size: 13)).foregroundStyle(.secondary)
                        .frame(maxWidth: .infinity, alignment: .leading)
                } else {
                    LazyVStack(alignment: .leading, spacing: 0) {
                        ForEach(Array(selectedHistory.enumerated()), id: \.element.id) { index, item in
                            if let date = item.timestamp,
                               AgentTaskHistoryPresentation.startsNewDay(at: index, in: selectedHistory) {
                                dayDivider(for: date)
                            }
                            historyRow(item)
                        }
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
            }
            .padding(10)
            .background(Color(nsColor: .textBackgroundColor), in: RoundedRectangle(cornerRadius: 8))
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
    }

    private func dayDivider(for date: Date) -> some View {
        HStack(spacing: 10) {
            Rectangle().fill(Color.secondary.opacity(0.35)).frame(height: 1)
            Text(AgentTaskHistoryPresentation.dateLabel(date))
                .font(.system(size: 12, weight: .medium)).foregroundStyle(.secondary)
                .fixedSize()
            Rectangle().fill(Color.secondary.opacity(0.35)).frame(height: 1)
        }
        .padding(.vertical, 14)
    }

    private func historyRow(_ item: AgentTaskHistoryItem) -> some View {
        HStack(alignment: .top, spacing: 10) {
            Image(systemName: item.icon.systemImage)
                .font(.system(size: 17, weight: .medium))
                .symbolRenderingMode(.hierarchical)
                .foregroundStyle(iconColor(for: item.icon))
                .frame(width: 24, height: 24)
                .accessibilityLabel(item.icon.accessibilityLabel)
                .help(item.icon.accessibilityLabel)
            VStack(alignment: .leading, spacing: 4) {
                HStack(alignment: .firstTextBaseline) {
                    Text(item.title).font(.system(size: 14, weight: .semibold))
                    Spacer(minLength: 8)
                    Text(item.timestamp.map { AgentTaskHistoryPresentation.timeLabel($0) } ?? "時刻不明")
                        .font(.system(size: 12, design: .monospaced)).foregroundStyle(.secondary)
                }
                if item.eventType != "state_updated" {
                    Text(item.detail)
                        .font(.system(size: 13, design: .monospaced))
                        .textSelection(.enabled)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.vertical, 8)
    }

    private func iconColor(for icon: AgentTaskHistoryIcon) -> Color {
        switch icon {
        case .pending, .awaitingApproval, .awaitingInput: .orange
        case .running: .blue
        case .completed: .green
        case .failed: .red
        case .started, .observation, .unknown: .secondary
        }
    }
}

struct AgentTaskStatusLabel: View {
    let status: String
    let updatedAt: String?
    var showsStatusName = false
    var showsUnknownTime = false

    private var icon: AgentTaskHistoryIcon { AgentTaskHistoryIcon(status: status) }
    private var timestamp: String? {
        if showsUnknownTime {
            return AgentTaskHistoryPresentation.taskUpdateTimeLabel(updatedAt)
        }
        guard let updatedAt, let date = AgentTaskHistoryPresentation.parseTimestamp(updatedAt) else { return nil }
        return AgentTaskHistoryPresentation.timeLabel(date)
    }

    var body: some View {
        HStack(spacing: 5) {
            Image(systemName: icon.systemImage)
                .symbolRenderingMode(.hierarchical)
            if showsStatusName {
                Text(icon.accessibilityLabel)
            }
            if let timestamp {
                Text("· \(timestamp)").foregroundStyle(.secondary)
            }
        }
        .font(.system(size: 13))
        .foregroundStyle(color)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(Text([icon.accessibilityLabel, timestamp].compactMap { $0 }.joined(separator: " · ")))
        .help(icon.accessibilityLabel)
    }

    private var color: Color {
        switch icon {
        case .pending, .awaitingApproval, .awaitingInput: .orange
        case .running: .blue
        case .completed: .green
        case .failed: .red
        case .started, .observation, .unknown: .secondary
        }
    }
}
