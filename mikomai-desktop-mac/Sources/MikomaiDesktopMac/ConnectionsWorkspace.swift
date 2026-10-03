import SwiftUI
import AppKit
import Foundation
import Darwin
import Security
import CryptoKit
import MikomaiFFI
import MikomaiDesktopCore
import UniformTypeIdentifiers

// MARK: - Connections Workspace

struct ConnectionsWorkspace: View {
    @ObservedObject var model: DesktopModel
    @State private var csvAlert = ""
    @State private var showsCSVAlert = false

    var body: some View {
        VStack(spacing: 0) {
            HStack {
                Text("機器情報一覧").font(.system(size: 15, weight: .semibold))
                Spacer()
                AccessibleButton("機器を追加") { model.editingConnection = SavedConnection(name: "", host: "") } label: { Label("機器を追加", systemImage: "plus") }
                    .accessibleButtonStyle(.prominent).controlSize(.small)
            }.padding(16)
            Divider()
            if model.connections.isEmpty {
                VStack(spacing: 9) {
                    Image(systemName: "point.3.connected.trianglepath.dotted").font(.system(size: 24)).foregroundStyle(.secondary)
                    Text("登録した機器はありません").font(.system(size: 16, weight: .semibold))
                    Text("ネットワーク機器の接続情報を登録できます。資格情報はKeychainに安全に保存します。").font(.system(size: 14)).foregroundStyle(.secondary)
                }.frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                Table(model.connections) {
                    TableColumn("名前", value: \.name)
                    TableColumn("ホスト", value: \.host)
                    TableColumn("ポート", value: \.port).width(50)
                    TableColumn("ユーザー", value: \.username)
                    TableColumn("機器タイプ") { connection in
                        Text(DeviceTypeCatalog.displayName(for: connection.deviceType))
                    }
                    TableColumn("資格情報") { connection in
                        if connection.hasPassword || connection.hasEnablePassword {
                            Label(
                                connection.hasPassword && connection.hasEnablePassword ? "Key + Enable" :
                                    (connection.hasEnablePassword ? "Enable" : "Key"),
                                systemImage: "key.fill"
                            ).font(.system(size: 13)).foregroundStyle(.green)
                        } else {
                            Text("未設定").font(.system(size: 13)).foregroundStyle(.secondary)
                        }
                    }.width(90)
                    TableColumn("操作") { connection in
                        HStack(spacing: 6) {
                            AccessibleButton("機器を編集: \(connection.name)") {
                                model.editingConnection = connection
                            } label: {
                                Image(systemName: "pencil")
                            }
                            .help("編集").accessibilityLabel("機器を編集: \(connection.name)")

                            AccessibleButton("機器を削除: \(connection.name)", role: .destructive) {
                                model.deleteConnection(connection.id)
                            } label: {
                                Image(systemName: "trash")
                            }
                            .help("削除").accessibilityLabel("機器を削除: \(connection.name)")
                        }
                        .accessibleButtonStyle(.plain)
                        .accessibilityElement(children: .contain)
                    }.width(115)
                }
            }
            Spacer(minLength: 0)
            HStack {
                Text("資格情報は macOS Keychain に暗号化保存されます。CSV 形式での入出力に対応しています。")
                    .font(.system(size: 13)).foregroundStyle(.secondary)
                Spacer()
                AccessibleButton("CSV を読み込む") { importCSV() }
                AccessibleButton("旧 JSON を読み込む") { importLegacyRegistry() }
                AccessibleButton("CSV を書き出す") { exportCSV() }.disabled(model.connections.isEmpty)
            }.padding(12).background(Color(nsColor: .controlBackgroundColor).opacity(0.5))
        }
        .alert("機器情報", isPresented: $showsCSVAlert) { Button("OK", role: .cancel) {} } message: { Text(csvAlert) }
    }

    private func importLegacyRegistry() {
        let panel = NSOpenPanel()
        panel.allowedContentTypes = [.json]
        panel.canChooseFiles = true
        panel.canChooseDirectories = false
        panel.allowsMultipleSelection = false
        FilePanelPresenter.present(panel) { [self] response in
            guard response == .OK, let url = panel.url else { return }
            let scoped = url.startAccessingSecurityScopedResource()
            defer { if scoped { url.stopAccessingSecurityScopedResource() } }
            let json = url.path.withCString { path in
                let response = mikomai_device_registry_read(path)
                defer { mikomai_result_free(response) }
                guard let message = response.message else { return "エラー: 機器情報を読み込めませんでした。" }
                let text = String(cString: message)
                return response.status == 0 ? text : "エラー: \(text)"
            }
            guard !json.hasPrefix("エラー:") else {
                presentCSVMessage(json)
                return
            }
            let result: LegacyConnectionImportResult
            do {
                result = try model.importLegacyDevices(fromJSON: Data(json.utf8))
            } catch {
                presentCSVMessage("旧形式の機器情報 JSON を読み取れませんでした。元ファイルは変更していません。")
                return
            }
            var note = "\(result.imported.count) 件を追加し、\(result.skipped) 件をスキップしました。元ファイルは変更していません。"
            if result.missingIDs > 0 { note += " IDのない行は重複判定せず追加しました。" }
            presentCSVMessage(note)
        }
    }

    private func exportCSV() {
        let panel = NSSavePanel(); panel.nameFieldStringValue = "connections.csv"
        FilePanelPresenter.present(panel) { [self] response in
            guard response == .OK, let url = panel.url else { return }
            if let invalid = model.connections.first(where: { $0.validationError != nil }) {
                presentCSVMessage("\(invalid.name) のホスト名が 旧 CSV 形式の制約に合いません。機器情報を編集してください。")
                return
            }
            do {
                let csv = try ConnectionCSVCodec.exportCSV(model.connections)
                try csv.write(to: url, atomically: true, encoding: .utf8)
            } catch {
                presentCSVMessage("CSV ファイルを書き込めませんでした。\(error.localizedDescription)")
            }
        }
    }

    private func importCSV() {
        let panel = NSOpenPanel()
        panel.allowedContentTypes = [.commaSeparatedText]
        panel.allowsMultipleSelection = false
        FilePanelPresenter.present(panel) { [self] response in
            guard response == .OK, let url = panel.url,
                  let content = try? String(contentsOf: url, encoding: .utf8) else { return }
            do {
                let result = try ConnectionCSVCodec.importCSV(content, existing: model.connections)
                model.connections = result.connections
                let details = result.warnings.prefix(5).map { "\($0.row)行目: \($0.reason)" }
                var message = "\(result.importedCount) 件を読み込みました。\(result.warnings.count) 件は形式が合わないためスキップしました。"
                if !details.isEmpty { message += "\n" + details.joined(separator: "\n") }
                if result.warnings.count > details.count { message += "\nほか \(result.warnings.count - details.count) 件" }
                presentCSVMessage(message)
            } catch {
                presentCSVMessage(error.localizedDescription)
            }
        }
    }

    private func presentCSVMessage(_ message: String) {
        csvAlert = message
        showsCSVAlert = true
    }

}

// MARK: - Connection Editor with Keychain

struct ConnectionEditor: View {
    @Environment(\.dismiss) private var dismiss
    @State var connection: SavedConnection
    @State private var password = ""
    @State private var enablePassword = ""
    let onSave: (SavedConnection, String?, String?) -> Void
    @State private var showsDeviceTypes = false
    @State private var deviceTypeQuery = ""

    init(connection: SavedConnection, onSave: @escaping (SavedConnection, String?, String?) -> Void) {
        self._connection = State(initialValue: connection)
        self.onSave = onSave
        let credentials = ConnectionCredentialPersistence(store: KeychainCredentialAdapter()).load(for: connection.id)
        self._password = State(initialValue: credentials.password ?? "")
        self._enablePassword = State(initialValue: credentials.enablePassword ?? "")
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text(connection.name.isEmpty ? "機器を追加" : "機器情報を編集").font(.system(size: 18, weight: .semibold))
            Form {
                TextField("名前", text: $connection.name)
                TextField("ホスト名または IP", text: $connection.host)
                TextField("ポート", text: $connection.port)
                TextField("ユーザー名", text: $connection.username)
                AccessiblePicker("接続方式", selection: Binding(
                    get: { connection.connectionType ?? "SSH" },
                    set: { connection.selectConnectionType($0) }
                ), options: ["SSH", "Telnet", "Console"].map { ($0, $0) })
                LabeledContent("機器タイプ") {
                    AccessibleButton("機器タイプを選択", value: DeviceTypeCatalog.displayName(for: connection.deviceType)) {
                        deviceTypeQuery = ""
                        showsDeviceTypes = true
                    } label: {
                        HStack {
                            Text(DeviceTypeCatalog.displayName(for: connection.deviceType))
                            Image(systemName: "chevron.up.chevron.down")
                        }
                    }
                    .sheet(isPresented: $showsDeviceTypes) {
                        VStack(spacing: 8) {
                            Text("機器タイプを選択").font(.headline)
                            TextField("機器タイプを検索", text: $deviceTypeQuery)
                                .textFieldStyle(.roundedBorder)
                            ScrollView {
                                VStack(spacing: 2) {
                                    ForEach(DeviceTypeCatalog.matching(deviceTypeQuery), id: \.self) { id in
                                        HistorySelectionRow(
                                            DeviceTypeCatalog.optionLabel(for: id), isSelected: DeviceTypeCatalog.canonicalID(for: connection.deviceType) == id,
                                            action: {
                                                connection.deviceType = id
                                                showsDeviceTypes = false
                                            }
                                        ) {
                                            Text(DeviceTypeCatalog.optionLabel(for: id))
                                        }
                                    }
                                    if DeviceTypeCatalog.matching(deviceTypeQuery).isEmpty {
                                        Text("一致する機器タイプがありません")
                                            .foregroundStyle(.secondary).padding()
                                    }
                                }
                            }
                            HStack {
                                Spacer()
                                AccessibleButton("機器タイプの選択をキャンセル") { showsDeviceTypes = false }
                                    .accessibleCancelAction()
                            }
                        }
                        .padding(12).frame(width: 360, height: 380)
                        .background(KeyboardNavigationScope())
                    }
                }

                Section("資格情報 (Keychain)") {
                    SecureField("パスワード", text: $password)
                    SecureField("Enable パスワード", text: $enablePassword)
                    Text("パスワードは macOS Keychain に暗号化されて安全に保管されます。平文ファイルには保存されません。")
                        .font(.system(size: 12)).foregroundStyle(.secondary)
                }
            }
            .formStyle(.grouped)
            if let validationError = connection.validationError {
                Text(validationError).font(.system(size: 13)).foregroundStyle(.red)
            }
            HStack {
                Spacer()
                AccessibleButton("キャンセル") { dismiss() }.accessibleCancelAction()
                AccessibleButton("保存") {
                    guard connection.validationError == nil else { return }
                    onSave(connection, password, enablePassword)
                    dismiss()
                }
                .accessibleDefaultAction()
                .disabled(connection.validationError != nil)
            }
        }.padding(18).frame(width: 440, height: 440)
        .background(KeyboardNavigationScope())
    }
}
