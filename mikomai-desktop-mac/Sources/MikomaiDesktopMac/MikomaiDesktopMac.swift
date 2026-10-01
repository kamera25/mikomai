import SwiftUI
import AppKit

// SwiftPM launches an unbundled executable. Explicitly register it as a
// foreground app so its windows can receive keyboard and IME events.
private final class DesktopAppDelegate: NSObject, NSApplicationDelegate {
    func applicationWillFinishLaunching(_ notification: Notification) {
        // A second embedded database owner would prevent Agent startup.
        if let bundleID = Bundle.main.bundleIdentifier,
           let running = NSRunningApplication.runningApplications(withBundleIdentifier: bundleID)
            .first(where: { $0.processIdentifier != ProcessInfo.processInfo.processIdentifier && !$0.isTerminated }) {
            running.activate(options: [.activateAllWindows])
            NSApplication.shared.terminate(nil)
            return
        }
        NSApplication.shared.setActivationPolicy(.regular)
        if let iconURL = Bundle.module.url(forResource: "AppIcon", withExtension: "icns"),
           let icon = NSImage(contentsOf: iconURL) {
            NSApplication.shared.applicationIconImage = icon
        }
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApplication.shared.activate(ignoringOtherApps: true)
    }
}
@main
struct MikomaiDesktopMac: App {
    @NSApplicationDelegateAdaptor(DesktopAppDelegate.self) private var appDelegate
    @StateObject private var model = DesktopModel()

    var body: some Scene {
        WindowGroup {
            DesktopWindow(model: model)
                .frame(minWidth: 1020, minHeight: 680)
        }
        .windowStyle(.hiddenTitleBar)
        .commands {
            CommandGroup(replacing: .newItem) {
                Button("新しい会話") {
                    model.createSession()
                }
                .keyboardShortcut("n", modifiers: .command)
            }
            CommandMenu("表示") {
                Button("チャット") { model.workspace = .chat }.keyboardShortcut("1", modifiers: .command)
                Button("機器情報一覧") { model.workspace = .connections }.keyboardShortcut("2", modifiers: .command)
                Button("ネットワークツール") { model.workspace = .tools }.keyboardShortcut("3", modifiers: .command)
                Button("設定") { model.workspace = .settings }.keyboardShortcut("4", modifiers: .command)
                Button("監視・タスク履歴") { model.workspace = .monitoring }.keyboardShortcut("5", modifiers: .command)
            }
            CommandMenu("ネットワーク") {
                Button("接続テスト") {
                    model.workspace = .tools
                    model.selectedToolTab = .tcpTest
                }
                Button("Ping / Trace を実行") {
                    model.workspace = .tools
                    model.selectedToolTab = .ping
                }
                Button("ARP テーブル表示") {
                    model.workspace = .tools
                    model.selectedToolTab = .arp
                }
                Button("ルーティングテーブル表示") {
                    model.workspace = .tools
                    model.selectedToolTab = .route
                }
            }
        }
    }
}
