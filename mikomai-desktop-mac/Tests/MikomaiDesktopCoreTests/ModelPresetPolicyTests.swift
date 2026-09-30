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
}
