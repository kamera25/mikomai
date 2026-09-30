import SwiftUI
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
                        .font(.system(size: level == 1 ? 21 : level == 2 ? 18 : 15, weight: .semibold))
                        .padding(.top, level <= 2 ? 5 : 2)
                case let .paragraph(content):
                    inline(content).font(.system(size: 13)).lineSpacing(3)
                case let .code(language, content):
                    VStack(alignment: .leading, spacing: 6) {
                        HStack {
                            if !language.isEmpty { Text(language).font(.system(size: 10, weight: .medium)).foregroundStyle(.secondary) }
                            Spacer()
                            Button("コピー") {
                                ChatMessageClipboard.copy(text: content)
                            }
                            .buttonStyle(.borderless)
                            .font(.system(size: 10))
                            .foregroundStyle(.secondary)
                            Button("変更計画として確認") { onSelectConfig(content) }
                                .buttonStyle(.borderless).font(.system(size: 10))
                                .disabled(content.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                        }
                        Text(content).font(.system(size: 12, design: .monospaced)).textSelection(.enabled)
                            .frame(maxWidth: .infinity, alignment: .leading)
                    }
                    .padding(10).background(Color(nsColor: .controlBackgroundColor), in: RoundedRectangle(cornerRadius: 6))
                case let .bullet(content):
                    HStack(alignment: .firstTextBaseline, spacing: 8) {
                        Text("•").foregroundStyle(.secondary)
                        inline(content).font(.system(size: 13)).lineSpacing(3)
                    }.padding(.leading, 4)
                case let .quote(content):
                    inline(content).font(.system(size: 13)).foregroundStyle(.secondary)
                        .padding(.leading, 10).overlay(alignment: .leading) { Rectangle().fill(Color.accentColor.opacity(0.45)).frame(width: 2) }
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
