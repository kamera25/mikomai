public struct ModelPreset: Identifiable, Equatable, Sendable {
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
    public static let presets: [ModelPreset] = [
        ModelPreset(id: "gemma-4-e4b-ud", name: "Gemma 4 E4B (軽量・標準推奨)", repo: "unsloth/gemma-4-E4B-it-GGUF", filename: "gemma-4-E4B-it-UD-Q4_K_XL.gguf", mmprojFilename: "mmproj-F16.gguf"),
        ModelPreset(id: "gemma-4-12b-ud", name: "Gemma 4 12B (高精度)", repo: "unsloth/gemma-4-12b-it-GGUF", filename: "gemma-4-12b-it-UD-Q4_K_XL.gguf", mmprojFilename: "mmproj-F16.gguf"),
        ModelPreset(id: "gemma-4-e2b-ud", name: "Gemma 4 E2B (超軽量)", repo: "unsloth/gemma-4-E2B-it-GGUF", filename: "gemma-4-E2B-it-UD-Q4_K_XL.gguf", mmprojFilename: "mmproj-F16.gguf")
    ]

    public static func find(repo: String, filename: String) -> ModelPreset? {
        presets.first { $0.repo == repo && $0.filename == filename }
    }

    public static func find(filename: String) -> ModelPreset? {
        presets.first { $0.filename == filename }
    }
}
