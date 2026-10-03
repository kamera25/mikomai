import SwiftUI
import AppKit
import MikomaiDesktopCore

// MARK: - Markdown Message View

struct MarkdownMessage: View {
    let text: String
    var onSelectConfig: (String) -> Void = { _ in }
    private var blocks: [ChatMarkdownBlock] { ChatMarkdownParser.parse(text) }

    var body: some View {
        VStack(alignment: .leading, spacing: 9) {
            ForEach(Array(blocks.enumerated()), id: \.offset) { _, block in
                switch block.kind {
                case let .heading(level, content):
                    inline(content)
                        .font(.system(size: level == 3 ? 23 : level == 4 ? 20 : 17, weight: .semibold))
                        .padding(.top, level <= 2 ? 5 : 2)
                        .accessibilityAddTraits(.isHeader)
                case let .paragraph(content):
                    inline(content).font(.system(size: 15)).lineSpacing(3)
                case let .code(language, content):
                    VStack(alignment: .leading, spacing: 6) {
                        HStack {
                            if !language.isEmpty { Text(language).font(.system(size: 12, weight: .medium)).foregroundStyle(.secondary) }
                            Spacer()
                            AccessibleButton("コピー") {
                                ChatMessageClipboard.copy(text: content)
                            }
                            .accessibleButtonStyle(.plain)
                            .font(.system(size: 12))
                            .foregroundStyle(.secondary)
                            AccessibleButton("変更計画として確認") { onSelectConfig(content) }
                                .accessibleButtonStyle(.plain).font(.system(size: 12))
                                .disabled(content.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                        }
                        Text(content).font(.system(size: 14, design: .monospaced)).textSelection(.enabled)
                            .frame(maxWidth: .infinity, alignment: .leading)
                    }
                    .padding(10).background(Color(nsColor: .controlBackgroundColor), in: RoundedRectangle(cornerRadius: 6))
                case let .bullet(content):
                    HStack(alignment: .firstTextBaseline, spacing: 8) {
                        Text("•").foregroundStyle(.secondary)
                        inline(content).font(.system(size: 15)).lineSpacing(3)
                    }.padding(.leading, 4)
                case let .quote(content):
                    inline(content).font(.system(size: 15)).foregroundStyle(.secondary)
                        .padding(.leading, 10).overlay(alignment: .leading) { Rectangle().fill(Color.accentColor.opacity(0.45)).frame(width: 2) }
                case let .image(title, source):
                    if let image = ChatDiagramImage(source: source) {
                        NetworkDiagramView(title: title, image: image)
                    }
                case let .imageFile(url):
                    Link(destination: url) {
                        Label("画像ファイル", systemImage: "photo")
                    }
                    .font(.system(size: 15))
                    .help(url.path)
                    .accessibilityLabel("画像ファイル: \(url.lastPathComponent)")
                    .environment(\.openURL, OpenURLAction { destination in
                        NSWorkspace.shared.open(destination) ? .handled : .discarded
                    })
                case .separator:
                    Divider()
                }
            }
        }
        .textSelection(.enabled)
    }

    private func inline(_ source: String) -> Text {
        if let attributed = try? AttributedString(markdown: source, options: .init(interpretedSyntax: .inlineOnlyPreservingWhitespace)) {
            return Text(attributed)
        }
        return Text(source)
    }

}
