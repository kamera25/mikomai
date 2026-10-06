import SwiftUI
import AppKit
import Foundation
import Darwin
import Security
import CryptoKit
import MikomaiDesktopCore
import UniformTypeIdentifiers

// MARK: - Connections Workspace

struct ConnectionsWorkspace: View {
    @ObservedObject var model: DesktopModel

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
                    TableColumn("名前") { connection in
                        Text(connection.name.isEmpty ? "未設定" : connection.name)
                            .frame(maxWidth: .infinity, minHeight: 24, alignment: .leading)
                            .keyboardReadable("\(connection.name)の名前", text: connection.name)
                    }
                    TableColumn("ホスト") { connection in
                        Text(connection.host.isEmpty ? "未設定" : connection.host)
                            .frame(maxWidth: .infinity, minHeight: 24, alignment: .leading)
                            .keyboardReadable("\(connection.name)のホスト", text: connection.host)
                    }
                    TableColumn("ポート") { connection in
                        Text(connection.port.isEmpty ? "未設定" : connection.port)
                            .frame(maxWidth: .infinity, minHeight: 24, alignment: .leading)
                            .keyboardReadable("\(connection.name)のポート", text: connection.port)
                    }.width(50)
                    TableColumn("ユーザー") { connection in
                        Text(connection.username.isEmpty ? "未設定" : connection.username)
                            .frame(maxWidth: .infinity, minHeight: 24, alignment: .leading)
                            .keyboardReadable("\(connection.name)のユーザー", text: connection.username)
                    }
                    TableColumn("機器タイプ") { connection in
                        Text(DeviceTypeCatalog.displayName(for: connection.deviceType))
                            .frame(maxWidth: .infinity, minHeight: 24, alignment: .leading)
                            .keyboardReadable("\(connection.name)の機器タイプ", text: DeviceTypeCatalog.displayName(for: connection.deviceType))
                    }
                    TableColumn("資格情報") { connection in
                        Group {
                            if connection.hasPassword || connection.hasEnablePassword {
                                Label(
                                    connection.hasPassword && connection.hasEnablePassword ? "Key + Enable" :
                                        (connection.hasEnablePassword ? "Enable" : "Key"),
                                    systemImage: "key.fill"
                                ).font(.system(size: 13)).foregroundStyle(.green)
                            } else {
                                Text("未設定").font(.system(size: 13)).foregroundStyle(.secondary)
                            }
                        }
                        .frame(maxWidth: .infinity, minHeight: 24, alignment: .leading)
                        .keyboardReadable("\(connection.name)の資格情報", text: connection.hasPassword && connection.hasEnablePassword ? "パスワードとEnableパスワードを設定済み" : connection.hasEnablePassword ? "Enableパスワードを設定済み" : connection.hasPassword ? "パスワードを設定済み" : "未設定")
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
                Text("接続情報は機器ごとに登録・編集できます。資格情報は macOS Keychain に暗号化保存されます。")
                    .font(.system(size: 13)).foregroundStyle(.secondary)
                Spacer()
            }.padding(12).background(Color(nsColor: .controlBackgroundColor).opacity(0.5))
        }
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
    private let isNewConnection: Bool

    init(connection: SavedConnection, onSave: @escaping (SavedConnection, String?, String?) -> Void) {
        self._connection = State(initialValue: connection)
        self.isNewConnection = connection.name.isEmpty
        self.onSave = onSave
        self._password = State(initialValue: "")
        self._enablePassword = State(initialValue: "")
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text(isNewConnection ? "機器を追加" : "機器情報を編集").font(.system(size: 18, weight: .semibold))
                .accessibilityAddTraits(.isHeader)
            ScrollView {
                VStack(alignment: .leading, spacing: 14) {
                    AccessibleTextField(title: "名前（必須）", text: $connection.name, help: inputHelp("必須項目。機器を識別する名前を、文字・数字と . - _ で入力してください。", field: "名前"))
                    AccessibleTextField(title: "ホスト名またはIPアドレス（必須）", text: $connection.host, help: inputHelp("必須項目。接続先のホスト名、IPv4またはIPv6アドレスを入力してください。", field: "ホスト"))
                    AccessibleTextField(title: "ポート", text: $connection.port, help: inputHelp("1から65535の数値。空欄の場合は接続方式の既定値、\(connection.defaultPort)を使用します。", field: "ポート"))
                    AccessibleTextField(title: "ユーザー名", text: $connection.username, help: inputHelp("接続時のユーザー名。省略可能です。128文字以内で入力してください。", field: "ユーザー名"))
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
                                AccessibleTextField(title: "機器タイプを検索", text: $deviceTypeQuery, help: "機器名またはベンダー名で候補を絞り込みます。Tabで候補に移動し、Enterで選択します。", defersTabNavigation: true)
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
                                                .keyboardReadable("検索結果", text: "一致する機器タイプがありません")
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

                    VStack(alignment: .leading, spacing: 14) {
                        Text("資格情報 (Keychain)").font(.headline).accessibilityAddTraits(.isHeader)
                        AccessibleTextField(title: "パスワード", text: $password, help: "接続時に使用するパスワード。省略可能です。入力内容は保護され、Keychainに保存されます。", isSecure: true)
                        AccessibleTextField(title: "Enable パスワード", text: $enablePassword, help: "特権モードで使用するパスワード。省略可能です。入力内容は保護され、Keychainに保存されます。", isSecure: true)
                        Text("パスワードは macOS Keychain に暗号化されて安全に保管されます。平文ファイルには保存されません。")
                            .font(.system(size: 12)).foregroundStyle(.secondary)
                            .keyboardReadable("資格情報の保存について", text: "パスワードはmacOS Keychainに暗号化して保存されます。")
                    }
                }.frame(maxWidth: .infinity, alignment: .leading).padding(2)
            }
            if let validationError = connection.validationError {
                Text(validationError).font(.system(size: 13)).foregroundStyle(.red)
                    .keyboardReadable("入力エラー", text: validationError)
            }
            HStack {
                Spacer()
                AccessibleButton("キャンセル") { dismiss() }.accessibleCancelAction()
                AccessibleButton("保存") {
                    guard connection.validationError == nil else { return }
                    onSave(connection, password.isEmpty ? nil : password, enablePassword.isEmpty ? nil : enablePassword)
                    dismiss()
                }
                .accessibleDefaultAction()
                .disabled(connection.validationError != nil)
            }
        }.padding(18).frame(width: 480, height: 620)
        .background(KeyboardNavigationScope())
    }
    private func inputHelp(_ description: String, field: String) -> String {
        guard let error = connection.validationError, error.hasPrefix(field) else { return description }
        return description + " 入力エラー: " + error
    }

}
