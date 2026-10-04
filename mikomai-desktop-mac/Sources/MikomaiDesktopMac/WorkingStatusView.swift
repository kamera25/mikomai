import SwiftUI

/// Claude Code 風に定期的にランダムな状態メッセージを表示するワーキングインジケータ
struct WorkingStatusView: View {
    let isCancelling: Bool

    /// ランダムに切り替わる状態候補リスト（約10〜12個）
    static let candidateStatuses: [String] = [
        "考え中…",
        "確認中…",
        "調べ中…",
        "検討中…",
        "瞑想中…",
        "思案中…",
        "探索中…",
        "整理中…",
        "推論中…",
        "吟味中…",
        "分析中…",
        "構想中…"
    ]

    @State private var currentStatus: String = candidateStatuses.first ?? "考え中…"

    var body: some View {
        HStack(spacing: 8) {
            ProgressView().controlSize(.small)
            Text(isCancelling ? "生成を停止しています…" : currentStatus)
                .font(.system(size: 14))
                .foregroundStyle(.secondary)
                .id(isCancelling ? "cancelling" : currentStatus)
                .transition(.opacity)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .task(id: isCancelling) {
            guard !isCancelling else { return }
            if let initial = Self.candidateStatuses.randomElement() {
                currentStatus = initial
            }
            while !Task.isCancelled {
                try? await Task.sleep(nanoseconds: 2_500_000_000)
                if Task.isCancelled { break }
                let available = Self.candidateStatuses.filter { $0 != currentStatus }
                if let next = available.randomElement() {
                    withAnimation(.easeInOut(duration: 0.25)) {
                        currentStatus = next
                    }
                }
            }
        }
    }
}
