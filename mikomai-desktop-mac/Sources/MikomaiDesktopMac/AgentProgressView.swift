import SwiftUI
import MikomaiDesktopCore

struct AgentProgressView: View {
    let goal: String
    let entries: [AgentProgressEntry]
    let isRunning: Bool

    var body: some View {
        if let current = entries.last {
            VStack(alignment: .leading, spacing: 12) {
                HStack(spacing: 8) {
                    if isRunning { ProgressView().controlSize(.small) }
                    Label(current.phase, systemImage: icon(for: current.phase))
                        .font(.subheadline.weight(.semibold))
                    Spacer()
                    Text("Agent").font(.caption).foregroundStyle(.secondary)
                }
                field("目的", text: goal)
                field("次のアクション", text: current.nextAction)
                Text(current.detail).font(.callout).foregroundStyle(.secondary).lineLimit(3)
                DisclosureGroup("実行内容 · \(entries.count)件") {
                    VStack(alignment: .leading, spacing: 12) {
                        ForEach(Array(entries.enumerated()), id: \.element.id) { index, entry in
                            VStack(alignment: .leading, spacing: 4) {
                                Label("\(index + 1). \(entry.phase)", systemImage: icon(for: entry.phase))
                                    .font(.caption.weight(.semibold))
                                Text(entry.detail).font(.callout).textSelection(.enabled)
                            }
                            .frame(maxWidth: .infinity, alignment: .leading)
                        }
                    }.padding(.top, 8)
                }
                .font(.callout)
            }
            .padding(14)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(Color(nsColor: .controlBackgroundColor), in: RoundedRectangle(cornerRadius: 12))
            .overlay(RoundedRectangle(cornerRadius: 12).stroke(Color.secondary.opacity(0.15)))
        }
    }

    private func field(_ title: String, text: String) -> some View {
        VStack(alignment: .leading, spacing: 3) {
            Text(title).font(.caption).foregroundStyle(.secondary)
            Text(text).font(.callout).fixedSize(horizontal: false, vertical: true).textSelection(.enabled)
        }
    }

    private func icon(for phase: String) -> String {
        switch phase {
        case "計画": return "list.bullet.clipboard"
        case "実行": return "terminal"
        case "結果整理": return "doc.text.magnifyingglass"
        case "承認待ち", "確認待ち": return "person.crop.circle.badge.questionmark"
        case "完了": return "checkmark.circle"
        case "失敗": return "exclamationmark.triangle"
        case "停止": return "stop.circle"
        default: return "play.circle"
        }
    }
}
