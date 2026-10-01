import SwiftUI
import AppKit
import UniformTypeIdentifiers
import MikomaiDesktopCore

struct ExecutionTerminalView: View {
    let results: [AgentToolResult]
    @State private var showsCopyConfirmation = false
    @State private var saveError: String?

    private var logText: String {
        results.map { result in
            "$ \(result.command)\n\(result.output.isEmpty ? "(出力なし)" : result.output)\n\(result.succeeded ? "終了 · 成功" : "終了 · 失敗")"
        }.joined(separator: "\n\n")
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack {
                Label("Ping / Traceroute", systemImage: "terminal")
                    .font(.system(size: 12, weight: .semibold))
                Spacer()
                Button {
                    showsCopyConfirmation = ChatMessageClipboard.copy(text: logText)
                } label: {
                    Image(systemName: showsCopyConfirmation ? "checkmark" : "doc.on.doc")
                        .frame(width: 26, height: 26)
                        .contentShape(Rectangle())
                }
                .help(showsCopyConfirmation ? "コピーしました" : "ログをコピー")
                .accessibilityLabel("ログをコピー")
                Button(action: saveLog) {
                    Image(systemName: "arrow.down.to.line")
                        .frame(width: 26, height: 26)
                        .contentShape(Rectangle())
                }
                .help("ログをファイルに保存")
                .accessibilityLabel("ログをファイルに保存")
            }
            .buttonStyle(.plain)
            .foregroundStyle(Color.white.opacity(0.8))
            .disabled(results.isEmpty)
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
        .onChange(of: logText) { _ in showsCopyConfirmation = false }
        .task(id: showsCopyConfirmation) {
            guard showsCopyConfirmation else { return }
            do { try await Task.sleep(for: .milliseconds(1200)) }
            catch { return }
            showsCopyConfirmation = false
        }
        .alert("ログを保存できませんでした", isPresented: Binding(
            get: { saveError != nil },
            set: { if !$0 { saveError = nil } }
        )) {
            Button("OK") { saveError = nil }
        } message: {
            Text(saveError ?? "")
        }
    }

    private func saveLog() {
        let text = logText
        guard !text.isEmpty else { return }
        let panel = NSSavePanel()
        panel.allowedContentTypes = [.plainText]
        panel.nameFieldStringValue = "ping-trace-log.txt"
        panel.canCreateDirectories = true
        guard panel.runModal() == .OK, let url = panel.url else { return }
        do { try text.write(to: url, atomically: true, encoding: .utf8) }
        catch { saveError = error.localizedDescription }
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
