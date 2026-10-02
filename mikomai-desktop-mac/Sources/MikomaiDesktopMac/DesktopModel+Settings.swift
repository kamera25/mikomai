import Foundation
import AppKit
import UniformTypeIdentifiers
import MikomaiDesktopCore
import MikomaiFFI

extension DesktopModel {
    // MARK: - Native Settings Management

    func loadSettings() {
        let (loadedSettings, url, source) = SettingsManager.load()
        self.settings = loadedSettings
        self.settingsFileURL = url
        self.isSettingsLoaded = source != nil

        if source != nil {
            self.settingsStatusMessage = source == "imported"
                ? "既存設定を読み込み、Swift版の保存先へ移行しました: \(url.path)"
                : "Swift版設定を読み込みました: \(url.path)"
            if let path = loadedSettings.modelPath, !path.isEmpty {
                let expanded = (path as NSString).expandingTildeInPath
                self.modelPath = expanded
                // Check if matching preset
                let fname = FilePathPolicy.defaultFilename(expanded)
                if let match = ModelPresetCatalog.find(filename: fname) {
                    self.selectedPresetId = match.id
                    self.repoPath = match.repo
                    self.modelFilename = match.filename
                } else {
                    self.selectedPresetId = "custom"
                    self.modelFilename = fname
                }
                if FileManager.default.fileExists(atPath: expanded) {
                    loadModel()
                }
            }
        } else {
            let savedPath = defaults.string(forKey: "mikomai.desktop.mac.modelPath") ?? ""
            self.modelPath = (savedPath as NSString).expandingTildeInPath
            self.settingsStatusMessage = "Swift版設定ファイルがありません。デフォルト値を使用しています: \(url.path)"
            if !self.modelPath.isEmpty && FileManager.default.fileExists(atPath: self.modelPath) {
                loadModel()
            }
        }

        applyInferenceParams()
    }

    func saveSettings() {
        var toSave = settings
        if !modelPath.isEmpty {
            var patch = DesktopSettingsPatch()
            patch.modelPath = .set(modelPath)
            toSave.merge(patch)
        }
        do {
            try SettingsManager.save(toSave)
            self.isSettingsLoaded = true
            self.settingsStatusMessage = "Swift版設定を保存しました: \(settingsFileURL.path)"
            applyInferenceParams()
        } catch {
            self.settingsStatusMessage = "設定の保存に失敗しました: \(error.localizedDescription)"
        }
    }

    func resetSettingsToDefault() {
        self.settings = AppSettings()
        saveSettings()
        applyInferenceParams()
        self.settingsStatusMessage = "設定をデフォルト値にリセットしました。"
    }

    func applyInferenceParams() {
        let temp = Float(settings.temperature)
        let rep = Float(settings.repetitionPenalty)
        let nCtx = UInt32(settings.nCtx)
        let maxGen = UInt32(settings.maxGen)
        _ = Self.callRust {
            mikomai_set_inference_params(temp, rep, nCtx, maxGen)
        }
    }

    func selectPreset(_ presetId: String) {
        selectedPresetId = presetId
        if presetId != "custom", let preset = PRESET_MODELS.first(where: { $0.id == presetId }) {
            repoPath = preset.repo
            modelFilename = preset.filename
            let cachedURL = HuggingFaceHub.modelURL(repo: preset.repo, filename: preset.filename)
            if FileManager.default.fileExists(atPath: cachedURL.path) {
                modelPath = cachedURL.path
            }
        }
    }

    // MARK: - Model Management

    func selectModel() {
        let panel = NSOpenPanel()
        if let gguf = UTType(filenameExtension: "gguf") { panel.allowedContentTypes = [gguf] }
        panel.canChooseFiles = true
        panel.canChooseDirectories = false
        panel.allowsMultipleSelection = false
        if panel.runModal() == .OK, let url = panel.url {
            modelPath = url.path
            settings.modelPath = url.path
            saveSettings()
        }
    }

    func loadModel() {
        let path = (modelPath as NSString).expandingTildeInPath
        guard !path.isEmpty, !isLoadingModel else { return }
        isLoadingModel = true
        modelStatus = "モデルを読み込み中…"
        applyInferenceParams()
        Task.detached(priority: .userInitiated) {
            let status = Self.callRust { path.withCString { mikomai_model_load($0) } }
            let loadedPath = Self.callRust { mikomai_model_status() }
            await MainActor.run {
                if status.hasPrefix("エラー") {
                    let current = loadedPath.isEmpty ? "" : " (現在: \(URL(fileURLWithPath: loadedPath).lastPathComponent))"
                    self.modelStatus = "\(status)\(current)"
                } else {
                    self.modelStatus = "読み込み済み: \(URL(fileURLWithPath: loadedPath).lastPathComponent)"
                }
                self.isLoadingModel = false
            }
        }
    }

    func refreshModelStatus() {
        let status = Self.callRust { mikomai_model_status() }
        if !status.isEmpty { modelStatus = "読み込み済み: \(URL(fileURLWithPath: status).lastPathComponent)" }
    }

    func openModelDirectory() {
        let dir = HuggingFaceHub.cacheDirectory
        NSWorkspace.shared.open(dir)
    }

    func openSettingsDirectory() {
        let dir = settingsFileURL.deletingLastPathComponent()
        NSWorkspace.shared.open(dir)
    }
}
