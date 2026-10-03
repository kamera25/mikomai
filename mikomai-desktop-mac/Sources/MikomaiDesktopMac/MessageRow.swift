import SwiftUI
import MikomaiDesktopCore

struct MessageRow: View {
    let message: ChatMessage
    var isRunning = false
    var onSelectConfig: (String) -> Void = { _ in }
    var onShowTraceResults: () -> Void = {}
    @State private var showsCopyConfirmation = false
    @State private var copyFeedbackGeneration = 0
    var body: some View {
        VStack(alignment: message.role == .user ? .trailing : .leading, spacing: 6) {
            Group {
                if message.role == .user {
                    HStack {
                        Spacer(minLength: 48)
                        VStack(alignment: .leading, spacing: 6) {
                            Text(message.text).font(.system(size: 15)).textSelection(.enabled)
                                .multilineTextAlignment(.leading)
                                .fixedSize(horizontal: false, vertical: true)
                            ForEach(message.attachments, id: \.self) { name in
                                Label(name, systemImage: "doc.text").font(.system(size: 13))
                                    .fixedSize(horizontal: false, vertical: true)
                            }
                        }
                        .foregroundStyle(.white)
                        .padding(.horizontal, 14).padding(.vertical, 10)
                        .background(Color.blue, in: RoundedRectangle(cornerRadius: 12))
                    }
                } else {
                    if !message.hasProbeResults, let entries = message.agentProgress, !entries.isEmpty {
                        AgentProgressView(goal: message.agentGoal ?? "", entries: entries, isRunning: isRunning)
                    }
                    if !message.conversationText.isEmpty {
                        MarkdownMessage(text: message.conversationText, onSelectConfig: onSelectConfig)
                            .frame(maxWidth: .infinity, alignment: .leading)
                    }
                    if message.hasProbeResults {
                        VStack(alignment: .leading, spacing: 6) {
                            ForEach(message.displayedProbeResults) { result in
                                if let name = result.probeDisplayName {
                                    Text("\(name)を実行しました。")
                                        .font(.system(size: 15))
                                    AccessibleButton("\(name)の結果を右ペインに表示", action: onShowTraceResults) {
                                        Label("\(name)の結果", systemImage: "terminal")
                                            .font(.system(size: 13))
                                            .contentShape(Rectangle())
                                    }
                                    .accessibleButtonStyle(.plain)
                                    .accessibilityLabel("\(name)の結果を右ペインに表示")
                                }
                            }
                        }
                    }
                }
            }
            AccessibleButton("メッセージをコピー") {
                guard ChatMessageClipboard.copy(text: message.conversationText) else { return }
                showsCopyConfirmation = true
                copyFeedbackGeneration += 1
            } label: {
                Image(systemName: showsCopyConfirmation ? "checkmark" : "doc.on.doc")
                    .font(.system(size: 14))
                    .foregroundStyle(.secondary)
                    .frame(width: 28, height: 26)
                    .contentShape(Rectangle())
            }
            .accessibleButtonStyle(.plain)
            .help("メッセージをコピー")
            .accessibilityLabel("メッセージをコピー")
            .disabled(message.conversationText.isEmpty)
            .overlay(alignment: message.role == .user ? .topTrailing : .topLeading) {
                if showsCopyConfirmation {
                    VStack(spacing: 0) {
                        Text("コピーしました")
                            .font(.system(size: 13))
                            .foregroundStyle(.primary)
                            .padding(.horizontal, 10).padding(.vertical, 6)
                            .background(Color(nsColor: .controlBackgroundColor), in: RoundedRectangle(cornerRadius: 7))
                        CopyConfirmationTail()
                            .fill(Color(nsColor: .controlBackgroundColor))
                            .frame(width: 10, height: 5)
                            .frame(maxWidth: .infinity, alignment: message.role == .user ? .trailing : .leading)
                            .padding(.horizontal, 9)
                    }
                    .fixedSize()
                    .offset(y: -34)
                    .allowsHitTesting(false)
                    .accessibilityHidden(true)
                }
            }
            .task(id: copyFeedbackGeneration) {
                guard showsCopyConfirmation else { return }
                do { try await Task.sleep(for: .milliseconds(1200)) }
                catch { return }
                showsCopyConfirmation = false
            }
            .onDisappear { showsCopyConfirmation = false }
        }
        .frame(maxWidth: .infinity, alignment: message.role == .user ? .trailing : .leading)
    }
}

private struct CopyConfirmationTail: Shape {
    func path(in rect: CGRect) -> Path {
        Path { path in
            path.move(to: CGPoint(x: rect.minX, y: rect.minY))
            path.addLine(to: CGPoint(x: rect.maxX, y: rect.minY))
            path.addLine(to: CGPoint(x: rect.midX, y: rect.maxY))
            path.closeSubpath()
        }
    }
}
