import Testing
@testable import MikomaiDesktopCore

@Suite
struct ModelPresetPolicyTests {
    @Test func shippedPresetCatalogHasThreeStableEntries() {
        #expect(ModelPresetCatalog.presets.count == 3)
        let first = ModelPresetCatalog.presets[0]
        #expect(ModelPresetCatalog.find(repo: first.repo, filename: first.filename)?.id == "gemma-4-e4b-ud")
    }

    @Test func customModelCoordinatesDoNotMatchAShippedPreset() {
        #expect(ModelPresetCatalog.find(repo: "custom/repository", filename: "custom.gguf") == nil)
    }

    @Test func afm3CoreRequiresMacOS27AndKeepsGGUFPresetsSeparate() {
        #expect(!AppleModelPolicy.supportsOS(majorVersion: 26))
        #expect(AppleModelPolicy.supportsOS(majorVersion: 27))
        #expect(AppleModelPolicy.supportsOS(majorVersion: 28))
        #expect(!ModelPresetCatalog.presets.contains { $0.id == AppleModelPolicy.presetID })
    }
}
