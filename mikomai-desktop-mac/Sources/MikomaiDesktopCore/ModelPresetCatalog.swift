public struct ModelPreset: Identifiable, Equatable, Sendable, Codable {
    public var id: String
    public var name: String
    public var repo: String
    public var filename: String
    public var mmprojFilename: String?

    public init(id: String, name: String, repo: String, filename: String, mmprojFilename: String? = nil) {
        self.id = id
        self.name = name
        self.repo = repo
        self.filename = filename
        self.mmprojFilename = mmprojFilename
    }
}

public enum ModelPresetCatalog {
    public static let presets: [ModelPreset] = RustPolicy.call(["op": "model_presets"])

    public static func find(repo: String, filename: String) -> ModelPreset? {
        presets.first { $0.repo == repo && $0.filename == filename }
    }

    public static func find(filename: String) -> ModelPreset? {
        presets.first { $0.filename == filename }
    }
}

/// AFM 3 Core is supplied by macOS, rather than downloaded as a GGUF preset.
public enum AppleModelPolicy {
    public static let presetID = "afm-3-core"
    public static let name = "AFM 3 Core"
    public static let unsupportedOSMessage = "このマシンのOSは非対応です。macOS 27以降にアップデートしてください。"

    public static func supportsOS(majorVersion: Int) -> Bool {
        majorVersion >= 27
    }
}
