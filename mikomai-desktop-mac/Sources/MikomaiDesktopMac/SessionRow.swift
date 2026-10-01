import SwiftUI
import MikomaiDesktopCore

struct SessionRow: View {
    let session: ChatSession
    let isSelected: Bool
    let onSelect: () -> Void
    let onRename: (String) -> Void
    let onDelete: () -> Void
    @State private var isRenaming = false
    @State private var title = ""

    var body: some View {
        HStack(spacing: 7) {
            Image(systemName: "bubble.left").font(.system(size: 11)).foregroundStyle(.secondary)
            if isRenaming {
                TextField("会話名", text: $title, onCommit: { onRename(title); isRenaming = false })
                    .textFieldStyle(.plain).font(.system(size: 12))
            } else {
                Button(action: onSelect) {
                    Text(session.title).font(.system(size: 12)).lineLimit(1).frame(maxWidth: .infinity, alignment: .leading)
                }.buttonStyle(.plain)
            }
        }
        .padding(.horizontal, 8).padding(.vertical, 6).background(isSelected ? Color(nsColor: .selectedContentBackgroundColor).opacity(0.16) : .clear, in: RoundedRectangle(cornerRadius: 5))
        .contentShape(Rectangle())
        .contextMenu {
            Button("名前を変更") { title = session.title; isRenaming = true }
            Button("削除", role: .destructive, action: onDelete)
        }
    }
}
