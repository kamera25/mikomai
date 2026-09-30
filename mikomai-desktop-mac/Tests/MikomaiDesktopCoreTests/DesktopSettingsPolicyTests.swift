import Foundation
import Testing
@testable import MikomaiDesktopCore

@Suite
struct DesktopSettingsPolicyTests {
    @Test func loadsTauriSettingsAndAppliesDefaultsForMissingValues() throws {
        let json = Data(#"""
        {
          "historyLimit": 8,
          "temperature": 0.5,
          "repetitionPenalty": 1.2,
          "modelPath": "/path/to/model.gguf",
          "recentIps": ["8.8.8.8"],
          "mcpTimeout": 20,
          "cacheExpiryMinutes": 5,
          "ipVersion": "ipv4",
          "consolePort": "COM3",
          "consoleBaudRate": 115200,
          "preloadKnowledge": false,
          "preloadAnalysis": false,
          "preloadRag": false
        }
        """#.utf8)

        let settings = try DesktopSettingsCodec.decode(json)

        #expect(settings.historyLimit == 8)
        #expect(settings.temperature == 0.5)
        #expect(settings.repetitionPenalty == 1.2)
        #expect(settings.modelPath == "/path/to/model.gguf")
        #expect(settings.recentIps == ["8.8.8.8"])
        #expect(settings.consolePort == "COM3")
        #expect(settings.consoleBaudRate == 115200)
        #expect(settings.nCtx == 8192)
        #expect(settings.visionEnabled == false)
    }

    @Test func partialSaveOverridePreservesOtherSettingsAndEncodesFullPayload() throws {
        var settings = DesktopSettings(
            historyLimit: 8,
            temperature: 0.5,
            repetitionPenalty: 1.2,
            modelPath: "/path/to/model.gguf",
            recentIps: ["8.8.8.8"],
            consolePort: "COM3",
            consoleBaudRate: 115200
        )

        settings.merge(DesktopSettingsPatch(temperature: .set(0.9)))
        let payload = try DesktopSettingsCodec.encode(settings)
        let decoded = try DesktopSettingsCodec.decode(payload)

        #expect(decoded.temperature == 0.9)
        #expect(decoded.historyLimit == 8)
        #expect(decoded.repetitionPenalty == 1.2)
        #expect(decoded.modelPath == "/path/to/model.gguf")
        #expect(decoded.recentIps == ["8.8.8.8"])
        #expect(decoded.consolePort == "COM3")
        #expect(decoded.consoleBaudRate == 115200)
    }
}
