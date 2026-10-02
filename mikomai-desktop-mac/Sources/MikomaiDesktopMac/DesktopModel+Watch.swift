import Foundation
import AppKit
import MikomaiFFI
import MikomaiDesktopCore

extension DesktopModel {
    // MARK: - Monitoring & Watch Lifecycle

    func startWatchService() {
        guard watchCallbackContext == nil else { refreshWatches(); return }
        let fm = FileManager.default
        let support = (fm.urls(for: .applicationSupportDirectory, in: .userDomainMask).first ?? fm.temporaryDirectory)
            .appendingPathComponent("MikomaiDesktopMac", isDirectory: true)
        let destination = support.appendingPathComponent("watches.json")
        let legacy = fm.homeDirectoryForCurrentUser.appendingPathComponent("Library/Application Support/com.mikomai.agent/watches.json")
        do {
            try fm.createDirectory(at: support, withIntermediateDirectories: true)
            if !fm.fileExists(atPath: destination.path), fm.fileExists(atPath: legacy.path) {
                try fm.copyItem(at: legacy, to: destination)
                watchStatus = "旧版の監視設定をSwift版へ移行しました"
            }
        } catch {
            watchStatus = "監視設定の移行に失敗しました: \(error.localizedDescription)"
            return
        }
        let box = WatchCallbackBox(connections: connections, credentialPersistence: credentialPersistence) { [weak self] data in
            Task { @MainActor [weak self] in
                guard let self, let value = try? JSONDecoder().decode(NativeWatch.Run.Notice.self, from: data) else { return }
                self.watchStatus = value.message
                self.watchAlert = WatchAlert(message: value.message)
                NSSound.beep()
                self.refreshWatches()
            }
        }
        let context = Unmanaged.passRetained(box).toOpaque()
        let response = destination.path.withCString { mikomai_watch_start($0, watchToolBridge, watchNotificationBridge, context) }
        defer { mikomai_result_free(response) }
        if response.status == 0 {
            watchCallbackBox = box
            watchCallbackContext = context
            if watchStatus == "監視サービス未起動" { watchStatus = "定期監視を実行中" }
            refreshWatches()
        } else {
            Unmanaged<WatchCallbackBox>.fromOpaque(context).release()
            watchStatus = response.message.map { String(cString: $0) } ?? "監視サービスを開始できませんでした"
        }
        refreshAgentTasks()
    }

    func stopWatchService() {
        guard let context = watchCallbackContext else { return }
        let response = mikomai_watch_stop()
        let succeeded = response.status == 0
        let message = response.message.map { String(cString: $0) }
        mikomai_result_free(response)
        guard succeeded else { watchStatus = message ?? "監視サービスの停止に失敗しました"; return }
        watchCallbackContext = nil
        watchCallbackBox = nil
        Unmanaged<WatchCallbackBox>.fromOpaque(context).release()
        watchStatus = message ?? "監視サービスを停止しました"
    }

    func refreshWatches() {
        let response = mikomai_watch_list()
        defer { mikomai_result_free(response) }
        guard response.status == 0, let text = response.message,
              let decoded = try? JSONDecoder().decode([NativeWatch].self, from: Data(String(cString: text).utf8)) else { return }
        watches = decoded
    }

    func createCPUWatch() {
        if watchCallbackContext == nil {
            startWatchService()
            guard watchCallbackContext != nil else { return }
        }
        guard !watchDevice.isEmpty, let interval = Int(watchInterval), interval > 0,
              let threshold = Double(watchThreshold), (0...100).contains(threshold) else {
            watchStatus = "機器、正の監視間隔、0〜100のしきい値を指定してください"; return
        }
        let ir: [String: Any] = [
            "version": 1, "schedule": ["every": "\(interval)s"],
            "steps": [
                ["id": "cpu", "call": "get_state", "args": ["device": watchDevice, "resource": "cpu"]],
                ["when": ["left": ["ref": "cpu.usage"], "operator": "gt", "right": threshold],
                 "then": [["call": "notify", "args": ["message": watchMessage]]]]
            ]
        ]
        guard let data = try? JSONSerialization.data(withJSONObject: ["name": watchName, "ir": ir]) else { return }
        let payload = String(decoding: data, as: UTF8.self)
        let response: MikomaiResult
        if let editingID = watchEditingID {
            response = editingID.withCString { id in payload.withCString { mikomai_watch_update(id, $0) } }
        } else {
            response = payload.withCString { mikomai_watch_create($0) }
        }
        let message = response.message.map { String(cString: $0) } ?? "監視設定を作成できませんでした"
        let ok = response.status == 0
        mikomai_result_free(response)
        watchStatus = ok ? (watchEditingID == nil ? "監視設定を作成しました" : "監視設定を更新しました") : message
        if ok { watchEditingID = nil }
        refreshWatches()
    }

    func editWatch(_ watch: NativeWatch) {
        watchEditingID = watch.id
        watchName = watch.name
        watchInterval = String(watch.ir.schedule.every.dropLast())
        for step in watch.ir.steps {
            switch step {
            case .call(let call): watchDevice = call.args.device
            case .when(let condition):
                watchThreshold = String(condition.when.right)
                if let notification = condition.then.first { watchMessage = notification.args.message }
            }
        }
    }

    func setWatch(_ watch: NativeWatch, enabled: Bool) {
        let response = watch.id.withCString { enabled ? mikomai_watch_enable($0) : mikomai_watch_disable($0) }
        let message = response.message.map { String(cString: $0) } ?? "更新できませんでした"
        watchStatus = response.status == 0 ? (enabled ? "監視を有効にしました" : "監視を停止しました") : message
        mikomai_result_free(response); refreshWatches()
    }

    func runWatch(_ watch: NativeWatch) {
        let response = watch.id.withCString { mikomai_watch_run_now($0) }
        watchStatus = Self.consumeRust(response)
        refreshWatches()
    }

    func deleteWatch(_ watch: NativeWatch) {
        let response = watch.id.withCString { mikomai_watch_delete($0) }
        watchStatus = Self.consumeRust(response); refreshWatches()
    }
}
