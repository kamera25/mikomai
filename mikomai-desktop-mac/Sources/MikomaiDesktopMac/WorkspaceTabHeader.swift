import SwiftUI

struct RightPaneTabHeader: View {
    let selectedTab: String
    let onSelect: (String) -> Void
    let onClose: () -> Void

    var body: some View {
        ViewThatFits(in: .horizontal) {
            row(showsTitles: true)
            row(showsTitles: false)
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 8)
        .background(Color(nsColor: .controlBackgroundColor))
    }

    private func row(showsTitles: Bool) -> some View {
        HStack(spacing: showsTitles ? 12 : 6) {
            WorkspaceTabButton(title: "Diff", icon: "arrow.left.arrow.right", isSelected: selectedTab == "diff", showsTitle: showsTitles) {
                onSelect("diff")
            }
            WorkspaceTabButton(title: "実行", icon: "terminal", isSelected: selectedTab == "execution", showsTitle: showsTitles) {
                onSelect("execution")
            }
            WorkspaceTabButton(title: "ログ", icon: "text.alignleft", isSelected: selectedTab == "logs", showsTitle: showsTitles) {
                onSelect("logs")
            }
            Spacer(minLength: 0)
            Button(action: onClose) {
                Image(systemName: "sidebar.right")
                    .font(.system(size: 15, weight: .medium))
                    .frame(width: 28, height: 26)
                    .background(Color.accentColor.opacity(0.15), in: RoundedRectangle(cornerRadius: 5))
            }
            .buttonStyle(.plain)
            .help("作業タブを閉じる")
            .accessibilityLabel("作業タブを閉じる")
        }
    }
}

struct WorkspaceTabButton: View {
    let title: String
    let icon: String
    let isSelected: Bool
    var showsTitle = true
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(spacing: 6) {
                Image(systemName: icon)
                if showsTitle { Text(title).lineLimit(1) }
            }
            .font(.system(size: 14, weight: isSelected ? .semibold : .regular))
            .fixedSize(horizontal: showsTitle, vertical: false)
            .padding(.horizontal, showsTitle ? 12 : 7).padding(.vertical, 7)
            .background(isSelected ? Color(nsColor: .selectedControlColor).opacity(0.18) : .clear, in: RoundedRectangle(cornerRadius: 6))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .help(title)
        .accessibilityLabel(title)
    }
}
