import Foundation
import AppKit
import UniformTypeIdentifiers
import MikomaiDesktopCore
import MikomaiBindings

extension DesktopModel {
    // MARK: - Native Settings Management

    func loadSettings() {
        isRestoringPersistence = true
        defer {
            isRestoringPersistence = false
            let directories = LegacyDataNotice.existingDirectories
            if !directories.isEmpty {
                settingsStatusMessage += "\n旧版のデータは読み込みません。不要な場合はFinderで次のディレクトリを確認して削除してください: " + directories.joined(separator: ", ")
            }
        }
        let loaded: (settings: AppSettings, url: URL, source: String?)
        do { loaded = try SettingsManager.load() }
        catch {
            settingsPersistenceAvailable = false
            settingsStatusMessage = "設定を読み込めませんでした: \(error.localizedDescription)"
            return
        }
        let (loadedSettings, url, source) = loaded
        self.settings = loadedSettings
        if let path = loadedSettings.documentsDirectory { documentsDirectory = (path as NSString).expandingTildeInPath }
        if let path = loadedSettings.knowledgeDirectory { knowledgeDirectory = (path as NSString).expandingTildeInPath }
        self.settingsFileURL = url
        self.isSettingsLoaded = source != nil

        if loadedSettings.llmBackend == .apple && supportsAppleModelOS {
            settingsStatusMessage = "SurrealDB の設定を読み込みました: \(url.path)"
            selectedPresetId = AppleModelPolicy.presetID
            modelPath = loadedSettings.modelPath ?? ""
            loadModel()
            return
        }
        if loadedSettings.llmBackend == .apple {
            // The option is hidden on older OS versions; restore GGUF instead.
            settings.llmBackend = .llamacpp
        }
        _ = Self.callRust { "llamacpp".withCString { mikomai_model_select_backend($0) } }

        if source != nil {
            self.settingsStatusMessage = "SurrealDB の設定を読み込みました: \(url.path)"
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
            self.modelPath = ""
            self.settingsStatusMessage = "保存済み設定がありません。デフォルト値を使用しています: \(url.path)"
            if !self.modelPath.isEmpty && FileManager.default.fileExists(atPath: self.modelPath) {
                loadModel()
            }
        }

        applyInferenceParams()
    }

    func persistSettingsPaths() {
        guard !isRestoringPersistence && settingsPersistenceAvailable else { return }
        var snapshot = settings
        snapshot.documentsDirectory = documentsDirectory
        snapshot.knowledgeDirectory = knowledgeDirectory
        snapshot.modelPath = modelPath.isEmpty ? nil : modelPath
        do { try SettingsManager.save(snapshot) }
        catch { persistenceError = "設定の保存に失敗しました: \(error.localizedDescription)" }
    }

    func saveSettings() {
        guard settingsPersistenceAvailable else {
            settingsStatusMessage = "読込みエラーのため、設定の上書きを停止しています。"
            return
        }
        var toSave = settings
        toSave.documentsDirectory = documentsDirectory
        toSave.knowledgeDirectory = knowledgeDirectory
        toSave.modelPath = modelPath.isEmpty ? nil : modelPath
        do {
            try SettingsManager.save(toSave)
            self.isSettingsLoaded = true
            self.settingsStatusMessage = "SurrealDB の設定を保存しました: \(settingsFileURL.path)"
            applyInferenceParams()
        } catch {
            self.settingsStatusMessage = "設定の保存に失敗しました: \(error.localizedDescription)"
        }
    }

    func resetSettingsToDefault() {
        self.settings = AppSettings()
        selectedPresetId = "custom"
        _ = Self.callRust { "llamacpp".withCString { mikomai_model_select_backend($0) } }
        refreshModelStatus()
        saveSettings()
        applyInferenceParams()
        self.settingsStatusMessage = "設定をデフォルト値にリセットしました。"
    }

    func applyInferenceParams() {
        guard !isAppleModelSelected else { return }
        let temp = Float(settings.temperature)
        let rep = Float(settings.repetitionPenalty)
        let nCtx = UInt32(settings.nCtx)
        let maxGen = UInt32(settings.maxGen)
        _ = Self.callRust {
            mikomai_set_inference_params(temp, rep, nCtx, maxGen)
        }
        let projector = ((settings.mmprojPath ?? "") as NSString).expandingTildeInPath
        let visionStatus = Self.callRust {
            projector.withCString { mikomai_configure_vision(settings.visionEnabled ? 1 : 0, $0) }
        }
        if visionStatus.hasPrefix("エラー") { settingsStatusMessage = visionStatus }
    }

    func selectPreset(_ presetId: String) {
        guard !isWorking && !isLoadingModel else { return }
        if presetId == AppleModelPolicy.presetID {
            guard supportsAppleModelOS else { return }
            selectedPresetId = presetId
            settings.llmBackend = .apple
            saveSettings()
            loadModel()
            return
        }
        if isAppleModelSelected {
            settings.llmBackend = .llamacpp
            _ = Self.callRust { "llamacpp".withCString { mikomai_model_select_backend($0) } }
            modelStatus = "モデル未ロード"
            refreshModelStatus()
            saveSettings()
        }
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
        guard !isWorking && !isLoadingModel else { return }
        let panel = NSOpenPanel()
        if let gguf = UTType(filenameExtension: "gguf") { panel.allowedContentTypes = [gguf] }
        panel.canChooseFiles = true
        panel.canChooseDirectories = false
        panel.allowsMultipleSelection = false
        FilePanelPresenter.present(panel) { [self] response in
            guard response == .OK, let url = panel.url else { return }
            selectPreset("custom")
            modelPath = url.path
            settings.modelPath = url.path
            saveSettings()
        }
    }

    func loadModel() {
        guard !isLoadingModel, !isWorking else { return }
        if isAppleModelSelected {
            guard supportsAppleModelOS else {
                modelStatus = AppleModelPolicy.unsupportedOSMessage
                return
            }
            isLoadingModel = true
            modelStatus = "AFM 3 Core の利用可否を確認中…"
            Task.detached(priority: .userInitiated) {
                let status = Self.callRust { "apple".withCString { mikomai_model_select_backend($0) } }
                await MainActor.run {
                    self.modelStatus = status.hasPrefix("エラー") ? status : "利用可能: AFM 3 Core"
                    self.isLoadingModel = false
                }
            }
            return
        }
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
        if isAppleModelSelected {
            modelStatus = status.hasPrefix("エラー") ? status : "利用可能: AFM 3 Core"
            return
        }
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
