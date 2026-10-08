import SwiftUI
import AppKit

// SwiftPM launches an unbundled executable. Explicitly register it as a
// foreground app so its windows can receive keyboard and IME events.
private final class DesktopAppDelegate: NSObject, NSApplicationDelegate {
    func applicationWillFinishLaunching(_ notification: Notification) {
        ProcessInfo.processInfo.processName = "Mikomai"
        // A second embedded database owner would prevent Agent startup.
        if let bundleID = Bundle.main.bundleIdentifier,
           let running = NSRunningApplication.runningApplications(withBundleIdentifier: bundleID)
            .first(where: { $0.processIdentifier != ProcessInfo.processInfo.processIdentifier && !$0.isTerminated }) {
            running.activate(options: [.activateAllWindows])
            NSApplication.shared.terminate(nil)
            return
        }
        NSApplication.shared.setActivationPolicy(.regular)
        // Packaged apps use the Info.plist icon. Replacing it at launch can
        // change its apparent size in the Dock; only unbundled SwiftPM runs
        // need an explicit icon.
        if Bundle.main.bundleURL.pathExtension != "app",
           let iconURL = Bundle.module.url(forResource: "AppIcon", withExtension: "icns"),
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
                .frame(minWidth: 520, idealWidth: 1120, minHeight: 560, idealHeight: 760)
        }
        .windowStyle(.hiddenTitleBar)
        .windowToolbarStyle(.unifiedCompact)
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
                Button("エージェント履歴") { model.workspace = .agentHistory }.keyboardShortcut("3", modifiers: .command)
                Button("設定") { model.workspace = .settings }.keyboardShortcut("4", modifiers: .command)
                Button("CPU監視") { model.workspace = .monitoring }.keyboardShortcut("5", modifiers: .command)
            }
        }
    }
}
