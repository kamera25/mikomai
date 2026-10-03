import SwiftUI
import AppKit
import UniformTypeIdentifiers
import MikomaiDesktopCore

struct CoreDebugView: View {
    @ObservedObject var model: DesktopModel
    @State private var search = ""
    @State private var saveError: String?
    @State private var followsOutput = true
    private var matches: [CoreDebugRecord] {
        model.debugRecords.filter { $0.matches(search) }
    }
    var body: some View {
        VStack(spacing: 8) {
            TextField("JSON・プロンプトを検索", text: $search).textFieldStyle(.roundedBorder)
            HStack {
                Text("\(matches.count) / \(model.debugRecords.count) 件").font(.caption)
                Spacer()
                Button("保存", action: save).disabled(model.debugRecords.isEmpty)
                Button("クリア") { model.debugRecords.removeAll() }.disabled(model.debugRecords.isEmpty)
            }
            Toggle("末尾に追従", isOn: $followsOutput).font(.caption)
            CoreDebugTerminal(
                text: model.debugRecords.isEmpty
                    ? "チャットを送信するとcoreとの送受信を表示します。\nログはアプリ終了時に消去されます。"
                    : matches.map(\.formatted).joined(separator: "\n\n"),
                followsOutput: followsOutput && search.isEmpty
            )

        }.padding(10)
        .alert("保存できませんでした", isPresented: Binding(get: { saveError != nil }, set: { if !$0 { saveError = nil } })) {
            Button("閉じる") { saveError = nil }
        } message: { Text(saveError ?? "") }
    }
    private func save() {
        // Export a complete snapshot, independent of the current search filter.
        let snapshot = CoreDebugRecord.export(model.debugRecords)
        let panel = NSSavePanel()
        panel.title = "デバッグログを保存（全件）"
        panel.nameFieldStringValue = "mikomai-debug.jsonl"
        panel.allowedContentTypes = [UTType(filenameExtension: "jsonl") ?? .plainText]
        FilePanelPresenter.present(panel) { response in
            guard response == .OK, let url = panel.url else { return }
            do { try snapshot.write(to: url, atomically: true, encoding: .utf8) }
            catch { saveError = error.localizedDescription }
        }
    }
}
