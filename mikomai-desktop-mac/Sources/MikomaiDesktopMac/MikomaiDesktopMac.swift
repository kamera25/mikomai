import SwiftUI
import MikomaiFFI

@main
struct MikomaiDesktopMac: App {
    var body: some Scene {
        WindowGroup {
            ChatWindow()
                .frame(minWidth: 760, minHeight: 560)
        }
        .windowStyle(.hiddenTitleBar)
    }
}

private struct ChatMessage: Identifiable {
    enum Role { case user, assistant }
    let id = UUID()
    let role: Role
    let text: String
}

@MainActor
private final class ChatModel: ObservableObject {
    @Published var messages: [ChatMessage] = []
    @Published var draft = ""
    @Published var isWorking = false

    func send() {
        let prompt = draft.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !prompt.isEmpty, !isWorking else { return }
        messages.append(ChatMessage(role: .user, text: prompt))
        draft = ""
        isWorking = true

        Task.detached(priority: .userInitiated) {
            let answer = Self.askRust(prompt)
            await MainActor.run {
                self.messages.append(ChatMessage(role: .assistant, text: answer))
                self.isWorking = false
            }
        }
    }

    private nonisolated static func askRust(_ prompt: String) -> String {
        guard let input = prompt.cString(using: .utf8) else {
            return "質問を UTF-8 に変換できませんでした。"
        }
        let response = input.withUnsafeBufferPointer { buffer in
            mikomai_chat(buffer.baseAddress!)
        }
        defer { mikomai_result_free(response) }
        guard let message = response.message else {
            return "Rust 側から応答がありませんでした。"
        }
        let text = String(cString: message)
        return response.status == 0 ? text : "エラー: \(text)"
    }
}

private struct ChatWindow: View {
    @StateObject private var model = ChatModel()

    var body: some View {
        HStack(spacing: 0) {
            sidebar
            VStack(spacing: 0) {
                header
                conversation
                composer
            }
            .background(Color(nsColor: .windowBackgroundColor))
        }
        .background(Color(nsColor: .underPageBackgroundColor))
    }

    private var sidebar: some View {
        VStack(alignment: .leading, spacing: 22) {
            HStack(spacing: 10) {
                Image(systemName: "point.3.connected.trianglepath.dotted")
                    .font(.system(size: 20, weight: .semibold))
                    .foregroundStyle(Color.accentColor)
                Text("mikomai")
                    .font(.system(size: 18, weight: .semibold))
            }
            Button(action: { model.messages.removeAll() }) {
                Label("新しい会話", systemImage: "square.and.pencil")
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
            .buttonStyle(.bordered)
            .controlSize(.large)
            Spacer()
            Label("ローカルナレッジ", systemImage: "books.vertical")
                .font(.system(size: 12, weight: .medium))
                .foregroundStyle(.secondary)
            Text("nw-docs")
                .font(.system(size: 12, design: .monospaced))
                .foregroundStyle(.secondary)
        }
        .padding(18)
        .frame(width: 218)
        .background(Color(nsColor: .controlBackgroundColor))
    }

    private var header: some View {
        HStack {
            VStack(alignment: .leading, spacing: 3) {
                Text("ネットワークアシスタント")
                    .font(.system(size: 15, weight: .semibold))
                Text("ローカル資料を検索して回答します")
                    .font(.system(size: 12))
                    .foregroundStyle(.secondary)
            }
            Spacer()
            Circle().fill(.green).frame(width: 7, height: 7)
            Text("準備完了")
                .font(.system(size: 11, weight: .medium))
                .foregroundStyle(.secondary)
        }
        .padding(.horizontal, 24)
        .padding(.vertical, 16)
        .background(Color(nsColor: .windowBackgroundColor))
        .overlay(alignment: .bottom) { Divider() }
    }

    private var conversation: some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 18) {
                    if model.messages.isEmpty {
                        emptyState
                    }
                    ForEach(model.messages) { message in
                        MessageRow(message: message)
                            .id(message.id)
                    }
                    if model.isWorking {
                        HStack(spacing: 8) {
                            ProgressView().controlSize(.small)
                            Text("資料を検索しています…")
                                .font(.system(size: 13))
                                .foregroundStyle(.secondary)
                        }
                        .padding(.leading, 48)
                    }
                }
                .frame(maxWidth: 760)
                .frame(maxWidth: .infinity)
                .padding(.horizontal, 28)
                .padding(.vertical, 28)
            }
            .onChange(of: model.messages.count) { _ in
                if let last = model.messages.last { proxy.scrollTo(last.id, anchor: .bottom) }
            }
        }
    }

    private var emptyState: some View {
        VStack(alignment: .leading, spacing: 14) {
            Image(systemName: "network")
                .font(.system(size: 26))
                .foregroundStyle(Color.accentColor)
            Text("ネットワークの資料を探す")
                .font(.system(size: 23, weight: .semibold))
            Text("機器の設定やトラブルシュートについて質問してください。")
                .font(.system(size: 14))
                .foregroundStyle(.secondary)
            HStack(spacing: 8) {
                suggestion("F220 の VLAN 設定")
                suggestion("Cisco の MAC アドレス確認")
            }
            .padding(.top, 7)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.top, 78)
    }

    private func suggestion(_ text: String) -> some View {
        Button(text) { model.draft = text }
            .buttonStyle(.bordered)
            .controlSize(.small)
    }

    private var composer: some View {
        HStack(alignment: .bottom, spacing: 12) {
            TextField("質問を入力…", text: $model.draft, axis: .vertical)
                .textFieldStyle(.plain)
                .lineLimit(1...5)
                .onSubmit(model.send)
                .disabled(model.isWorking)
            Button(action: model.send) {
                Image(systemName: "arrow.up")
                    .font(.system(size: 13, weight: .semibold))
                    .frame(width: 30, height: 30)
            }
            .buttonStyle(.borderedProminent)
            .controlSize(.small)
            .keyboardShortcut(.return, modifiers: [.command])
            .disabled(model.draft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || model.isWorking)
            .help("送信")
        }
        .padding(12)
        .background(Color(nsColor: .textBackgroundColor))
        .clipShape(RoundedRectangle(cornerRadius: 8))
        .overlay(RoundedRectangle(cornerRadius: 8).stroke(Color(nsColor: .separatorColor), lineWidth: 0.7))
        .frame(maxWidth: 760)
        .padding(.horizontal, 24)
        .padding(.top, 12)
        .padding(.bottom, 18)
        .frame(maxWidth: .infinity)
        .background(Color(nsColor: .windowBackgroundColor))
    }
}

private struct MessageRow: View {
    let message: ChatMessage

    var body: some View {
        HStack(alignment: .top, spacing: 12) {
            Image(systemName: message.role == .user ? "person.fill" : "point.3.connected.trianglepath.dotted")
                .font(.system(size: 13, weight: .semibold))
                .foregroundStyle(message.role == .user ? Color.secondary : Color.accentColor)
                .frame(width: 30, height: 30)
                .background(Color(nsColor: .controlBackgroundColor), in: RoundedRectangle(cornerRadius: 7))
            Text(message.text)
                .font(.system(size: 14))
                .textSelection(.enabled)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.top, 5)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}
