import SwiftUI
import MikomaiDesktopCore

struct ExecutionTerminalView: View {
    let results: [AgentToolResult]

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Label("Ping / Traceroute", systemImage: "terminal")
                .font(.system(size: 12, weight: .semibold))
                .foregroundStyle(Color.white.opacity(0.8))
                .padding(14)
            Divider().overlay(Color.white.opacity(0.15))
            GeometryReader { viewport in
                ScrollViewReader { proxy in
                    ScrollView([.vertical, .horizontal]) {
                        VStack(alignment: .leading, spacing: 0) {
                            VStack(alignment: .leading, spacing: 20) {
                                if results.isEmpty {
                                    Text("ping・tracerouteの実行結果がここに表示されます。")
                                        .foregroundStyle(Color.white.opacity(0.6))
                                }
                                ForEach(results) { result in
                                    VStack(alignment: .leading, spacing: 7) {
                                        Text("$ \(result.command)")
                                            .foregroundStyle(Color(red: 0.55, green: 0.87, blue: 0.67))
                                        Text(result.output.isEmpty ? "(出力なし)" : result.output)
                                            .foregroundStyle(Color.white.opacity(0.9))
                                        Label(result.succeeded ? "終了 · 成功" : "終了 · 失敗", systemImage: result.succeeded ? "checkmark.circle" : "exclamationmark.circle")
                                            .foregroundStyle(result.succeeded ? Color.green : Color.orange)
                                    }
                                    .fixedSize(horizontal: true, vertical: false)
                                    .textSelection(.enabled)
                                    .id(result.id)
                                }
                            }
                            .font(.system(size: 11, design: .monospaced))
                            .padding(14)
                            .frame(minWidth: viewport.size.width, alignment: .leading)
                            // A narrow target at x=0 avoids centering an oversized row.
                            Color.clear.frame(width: 1, height: 1).id("terminalBottomLeft")
                        }
                        .multilineTextAlignment(.leading)
                    }
                    .onAppear {
                        proxy.scrollTo("terminalBottomLeft", anchor: .bottomLeading)
                    }
                    .onChange(of: results.last?.id) { _ in
                        proxy.scrollTo("terminalBottomLeft", anchor: .bottomLeading)
                    }
                }
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background(Color(red: 0.07, green: 0.09, blue: 0.12))
    }
}

struct QueuedSubmissionView: View {
    let submission: QueuedChatSubmission
    let onRemove: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 7) {
            HStack {
                Label("次回送信予定", systemImage: "clock")
                    .font(.caption.weight(.semibold)).foregroundStyle(.secondary)
                Spacer()
                Button(action: onRemove) { Image(systemName: "xmark") }
                    .buttonStyle(.plain).foregroundStyle(.secondary)
                    .help("送信予定を取り消す")
                    .accessibilityLabel("送信予定を取り消す")
            }
            Text(submission.prompt.isEmpty ? "添付ファイルを確認してください。" : submission.prompt)
                .font(.callout).textSelection(.enabled)
            ForEach(submission.attachments) { attachment in
                Label(attachment.name, systemImage: "doc.text").font(.caption)
            }
        }
        .padding(12)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Color.accentColor.opacity(0.06), in: RoundedRectangle(cornerRadius: 10))
        .overlay(RoundedRectangle(cornerRadius: 10).stroke(Color.accentColor.opacity(0.2)))
    }
}
