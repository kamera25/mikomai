import SwiftUI
import AppKit
import Foundation
import Darwin
import Security
import CryptoKit
import MikomaiFFI
import MikomaiDesktopCore
import UniformTypeIdentifiers

// MARK: - Network Tools Workspace

struct NetworkToolsWorkspace: View {
    @ObservedObject var model: DesktopModel
    @StateObject private var diagnosticsRunner = DiagnosticsRunner()
    @State private var pingTarget = ""
    @State private var pingMode = "ping"
    @State private var pingCount = 4

    @State private var arpRecords: [ArpRecord] = []
    @State private var arpFilter = ""
    @State private var isLoadingArp = false

    @State private var routeRecords: [RouteRecord] = []
    @State private var routeFilter = ""
    @State private var isLoadingRoute = false

    var body: some View {
        VStack(spacing: 0) {
            tabBar
            Divider()
            switch model.selectedToolTab {
            case .tcpTest:
                tcpTestView
            case .ping:
                pingView
            case .arp:
                arpView
            case .route:
                routeView
            }
        }
        .onAppear {
            if model.tcpTestHost.isEmpty, let first = model.connections.first {
                model.tcpTestHost = first.host
            }
            if pingTarget.isEmpty, let first = model.connections.first {
                pingTarget = first.host
            }
        }
    }

    private var tabBar: some View {
        HStack(spacing: 12) {
            ForEach(ToolTab.allCases) { tab in
                WorkspaceTabButton(title: tab.rawValue, icon: tab.icon, isSelected: model.selectedToolTab == tab) {
                    model.selectedToolTab = tab
                }
            }
            Spacer()
        }
        .padding(.horizontal, 16).padding(.vertical, 8)
        .background(Color(nsColor: .controlBackgroundColor))
    }

    // MARK: 1. TCP Connection Test View

    private var tcpTestView: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                Text("TCP ポート接続テスト")
                    .font(.system(size: 15, weight: .semibold))

                Text("指定したホストおよびポートへの TCP ハンドシェイクを行い、到達可能性とレイテンシ（RTT）を計測します。")
                    .font(.system(size: 12)).foregroundStyle(.secondary)

                HStack(spacing: 10) {
                    VStack(alignment: .leading, spacing: 4) {
                        Text("ターゲット ホスト / IP").font(.system(size: 11, weight: .medium))
                        HStack {
                            TextField("192.168.1.1 または router.local", text: $model.tcpTestHost)
                                .textFieldStyle(.roundedBorder)

                            if !model.connections.isEmpty {
                                Menu {
                                    ForEach(model.connections) { conn in
                                        Button("\(conn.name) (\(conn.host))") {
                                            model.tcpTestHost = conn.host
                                            model.tcpTestPort = conn.port
                                        }
                                    }
                                } label: {
                                    Image(systemName: "list.bullet")
                                }
                                .menuStyle(.borderlessButton)
                                .help("登録機器から選択")
                            }
                        }
                    }
                    .frame(maxWidth: .infinity)

                    VStack(alignment: .leading, spacing: 4) {
                        Text("ポート").font(.system(size: 11, weight: .medium))
                        TextField("22", text: $model.tcpTestPort)
                            .textFieldStyle(.roundedBorder)
                            .frame(width: 80)
                    }

                    VStack(alignment: .leading, spacing: 4) {
                        Text("タイムアウト (ms)").font(.system(size: 11, weight: .medium))
                        TextField("2000", text: $model.tcpTestTimeout)
                            .textFieldStyle(.roundedBorder)
                            .frame(width: 90)
                    }

                    VStack(alignment: .leading, spacing: 4) {
                        Text(" ").font(.system(size: 11))
                        Button(action: model.testTcpDirect) {
                            HStack(spacing: 4) {
                                if model.isTestingTcp {
                                    ProgressView().controlSize(.small)
                                } else {
                                    Image(systemName: "bolt.fill")
                                }
                                Text("テスト実行")
                            }
                        }
                        .buttonStyle(.borderedProminent)
                        .disabled(model.tcpTestHost.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || model.isTestingTcp)
                    }
                }

                // Quick Port Presets
                HStack(spacing: 6) {
                    Text("プリセット:").font(.system(size: 11)).foregroundStyle(.secondary)
                    ForEach([("SSH", "22"), ("HTTP", "80"), ("HTTPS", "443"), ("Telnet", "23"), ("SNMP", "161"), ("Web (8080)", "8080")], id: \.1) { name, port in
                        Button("\(name) (\(port))") {
                            model.tcpTestPort = port
                        }
                        .buttonStyle(.bordered).controlSize(.mini)
                    }
                }

                // Result card
                if let result = model.tcpTestResult {
                    VStack(alignment: .leading, spacing: 8) {
                        HStack(spacing: 8) {
                            Circle()
                                .fill((model.tcpTestSuccess ?? false) ? Color.green : Color.red)
                                .frame(width: 10, height: 10)
                            Text((model.tcpTestSuccess ?? false) ? "接続成功" : "接続失敗")
                                .font(.system(size: 13, weight: .semibold))
                                .foregroundStyle((model.tcpTestSuccess ?? false) ? Color.green : Color.red)
                            Spacer()
                        }
                        Text(result)
                            .font(.system(size: 12, design: .monospaced))
                            .textSelection(.enabled)
                    }
                    .padding(12)
                    .background(Color(nsColor: .controlBackgroundColor), in: RoundedRectangle(cornerRadius: 8))
                    .overlay(RoundedRectangle(cornerRadius: 8).stroke(Color(nsColor: .separatorColor), lineWidth: 0.8))
                }

                // Recent tests log
                if !model.recentTcpTests.isEmpty {
                    VStack(alignment: .leading, spacing: 6) {
                        Text("最近のテスト結果").font(.system(size: 12, weight: .semibold)).foregroundStyle(.secondary)
                        VStack(alignment: .leading, spacing: 4) {
                            ForEach(model.recentTcpTests, id: \.self) { entry in
                                Text(entry)
                                    .font(.system(size: 11, design: .monospaced))
                                    .foregroundStyle(.secondary)
                            }
                        }
                        .padding(10)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .background(Color(nsColor: .controlBackgroundColor).opacity(0.6), in: RoundedRectangle(cornerRadius: 6))
                    }
                }
            }
            .padding(20)
            .frame(maxWidth: 820, alignment: .leading)
            .frame(maxWidth: .infinity, alignment: .topLeading)
        }
    }

    // MARK: 2. Ping / Traceroute View

    private var pingView: some View {
        VStack(spacing: 0) {
            HStack(spacing: 12) {
                Picker("", selection: $pingMode) {
                    Text("Ping").tag("ping")
                    Text("Traceroute").tag("traceroute")
                }
                .pickerStyle(.segmented)
                .frame(width: 180)

                TextField("ホスト名または IP アドレス", text: $pingTarget)
                    .textFieldStyle(.roundedBorder)

                if !model.connections.isEmpty {
                    Menu {
                        ForEach(model.connections) { conn in
                            Button("\(conn.name) (\(conn.host))") {
                                pingTarget = conn.host
                            }
                        }
                    } label: {
                        Image(systemName: "list.bullet")
                    }
                    .menuStyle(.borderlessButton)
                    .help("登録機器から選択")
                }

                if pingMode == "ping" {
                    Picker("回数", selection: $pingCount) {
                        Text("1回").tag(1)
                        Text("4回").tag(4)
                        Text("10回").tag(10)
                    }
                    .frame(width: 100)
                }

                if diagnosticsRunner.isRunning {
                    Button("停止", action: diagnosticsRunner.stop)
                        .buttonStyle(.borderedProminent)
                        .tint(.red)
                } else {
                    Button("実行") {
                        let target = pingTarget.trimmingCharacters(in: .whitespacesAndNewlines)
                        guard !target.isEmpty else { return }
                        if pingMode == "ping" {
                            diagnosticsRunner.run(command: "/sbin/ping", arguments: ["-c", "\(pingCount)", target])
                        } else {
                            diagnosticsRunner.run(command: "/usr/sbin/traceroute", arguments: ["-w", "2", "-m", "15", target])
                        }
                    }
                    .buttonStyle(.borderedProminent)
                    .disabled(pingTarget.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                }

                Button("クリア", action: diagnosticsRunner.clear)
                    .buttonStyle(.bordered)
                    .disabled(diagnosticsRunner.output.isEmpty)
            }
            .padding(14)
            .background(Color(nsColor: .windowBackgroundColor))
            Divider()

            ScrollViewReader { _ in
                ScrollView {
                    Text(diagnosticsRunner.output.isEmpty ? "Ping または Traceroute の実行結果がここにリアルタイムで表示されます。" : diagnosticsRunner.output)
                        .font(.system(size: 12, design: .monospaced))
                        .foregroundStyle(diagnosticsRunner.output.isEmpty ? .secondary : .primary)
                        .textSelection(.enabled)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(14)
                }
                .background(Color(nsColor: .textBackgroundColor))
            }
        }
    }

    // MARK: 3. ARP Table View

    private var arpView: some View {
        VStack(spacing: 0) {
            HStack {
                TextField("IP、MAC、インターフェースで検索…", text: $arpFilter)
                    .textFieldStyle(.roundedBorder)
                    .frame(maxWidth: 320)

                Spacer()

                Button("ARP テーブルを更新") {
                    isLoadingArp = true
                    Task.detached(priority: .userInitiated) {
                        let records = NetworkInspector.fetchArpTable()
                        await MainActor.run {
                            self.arpRecords = records
                            self.isLoadingArp = false
                        }
                    }
                }
                .buttonStyle(.bordered)
            }
            .padding(14)
            Divider()

            let filtered = arpRecords.filter { record in
                if arpFilter.isEmpty { return true }
                let query = arpFilter.lowercased()
                return record.ip.lowercased().contains(query)
                    || record.mac.lowercased().contains(query)
                    || record.interface.lowercased().contains(query)
            }

            if arpRecords.isEmpty {
                VStack(spacing: 8) {
                    Text("ARP テーブル未読み込み").font(.system(size: 13, weight: .semibold))
                    Text("上の「ARP テーブルを更新」ボタンをクリックして、ローカルマシンの ARP キャッシュを取得してください。").font(.system(size: 11)).foregroundStyle(.secondary)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                Table(filtered) {
                    TableColumn("IP アドレス", value: \.ip).width(min: 120, ideal: 140)
                    TableColumn("MAC アドレス", value: \.mac).width(min: 140, ideal: 160)
                    TableColumn("インターフェース", value: \.interface).width(80)
                    TableColumn("種別") { rec in
                        Text(rec.isPermanent ? "Permanent" : rec.isIncomplete ? "Incomplete" : "Dynamic")
                            .font(.system(size: 11))
                            .foregroundStyle(rec.isPermanent ? .blue : rec.isIncomplete ? .red : .secondary)
                    }.width(90)
                    TableColumn("アクション") { rec in
                        HStack(spacing: 6) {
                            Button("機器に追加") {
                                model.editingConnection = SavedConnection(name: rec.ip, host: rec.ip)
                            }
                            .buttonStyle(.borderless)
                            .font(.system(size: 11))

                            Button {
                                model.tcpTestHost = rec.ip
                                model.selectedToolTab = .tcpTest
                            } label: {
                                Image(systemName: "bolt.fill")
                            }
                            .buttonStyle(.borderless)
                            .help("接続テスト")
                        }
                    }.width(120)
                }
            }
        }
        .onAppear {
            if arpRecords.isEmpty {
                arpRecords = NetworkInspector.fetchArpTable()
            }
        }
    }

    // MARK: 4. Routing Table View

    private var routeView: some View {
        VStack(spacing: 0) {
            HStack {
                TextField("宛先、ゲートウェイ、インターフェースで検索…", text: $routeFilter)
                    .textFieldStyle(.roundedBorder)
                    .frame(maxWidth: 320)

                Spacer()

                Button("ルーティングテーブルを更新") {
                    isLoadingRoute = true
                    Task.detached(priority: .userInitiated) {
                        let records = NetworkInspector.fetchRoutingTable()
                        await MainActor.run {
                            self.routeRecords = records
                            self.isLoadingRoute = false
                        }
                    }
                }
                .buttonStyle(.bordered)
            }
            .padding(14)
            Divider()

            let filtered = routeRecords.filter { record in
                if routeFilter.isEmpty { return true }
                let query = routeFilter.lowercased()
                return record.destination.lowercased().contains(query)
                    || record.gateway.lowercased().contains(query)
                    || record.interface.lowercased().contains(query)
                    || record.flags.lowercased().contains(query)
            }

            if routeRecords.isEmpty {
                VStack(spacing: 8) {
                    Text("ルーティングテーブル未読み込み").font(.system(size: 13, weight: .semibold))
                    Text("上の「ルーティングテーブルを更新」ボタンをクリックして、ローカルマシンの経路情報を取得してください。").font(.system(size: 11)).foregroundStyle(.secondary)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                Table(filtered) {
                    TableColumn("宛先ネットワーク (Destination)", value: \.destination)
                    TableColumn("ゲートウェイ (Gateway)", value: \.gateway)
                    TableColumn("フラグ (Flags)", value: \.flags).width(70)
                    TableColumn("インターフェース (Netif)", value: \.interface).width(90)
                }
            }
        }
        .onAppear {
            if routeRecords.isEmpty {
                routeRecords = NetworkInspector.fetchRoutingTable()
            }
        }
    }
}

struct WorkspaceTabButton: View {
    let title: String
    let icon: String
    let isSelected: Bool
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(spacing: 6) {
                Image(systemName: icon)
                Text(title)
            }
            .font(.system(size: 12, weight: isSelected ? .semibold : .regular))
            .padding(.horizontal, 12).padding(.vertical, 7)
            .background(isSelected ? Color(nsColor: .selectedControlColor).opacity(0.18) : .clear, in: RoundedRectangle(cornerRadius: 6))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }
}

