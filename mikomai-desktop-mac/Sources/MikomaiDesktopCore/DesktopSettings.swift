import Foundation

public enum LLMBackend: String, Codable, Sendable {
    case llamacpp
    case apple
}

public struct DesktopSettings: Codable, Equatable {
    public var historyLimit: Int
    public var temperature: Double
    public var repetitionPenalty: Double
    public var modelPath: String?
    public var llmBackend: LLMBackend
    public var recentIps: [String]
    public var mcpTimeout: Int?
    public var ipVersion: String?
    public var consolePort: String?
    public var consoleBaudRate: Int?
    public var preloadKnowledge: Bool
    public var preloadAnalysis: Bool
    public var preloadRag: Bool
    public var preloadPlotter: Bool
    public var preloadBuilder: Bool
    public var preloadSummarization: Bool
    public var cacheExpiryMinutes: Int?
    public var nCtx: Int
    public var maxGen: Int
    public var promptKeepTokens: Int
    public var visionEnabled: Bool
    public var autoDryRun: Bool
    public var mmprojPath: String?

    public init(
        historyLimit: Int = 5, temperature: Double = 0.0, repetitionPenalty: Double = 1.1,
        modelPath: String? = nil, recentIps: [String] = [], mcpTimeout: Int? = 30,
        ipVersion: String? = "auto", consolePort: String? = nil, consoleBaudRate: Int? = 9600,
        preloadKnowledge: Bool = false, preloadAnalysis: Bool = false, preloadRag: Bool = false,
        preloadPlotter: Bool = false, preloadBuilder: Bool = false, preloadSummarization: Bool = false,
        cacheExpiryMinutes: Int? = 10, nCtx: Int = 8192, maxGen: Int = 2048,
        promptKeepTokens: Int = 500, visionEnabled: Bool = false, autoDryRun: Bool = false,
        mmprojPath: String? = nil, llmBackend: LLMBackend = .llamacpp
    ) {
        self.historyLimit = historyLimit
        self.temperature = temperature
        self.repetitionPenalty = repetitionPenalty
        self.modelPath = modelPath
        self.llmBackend = llmBackend
        self.recentIps = recentIps
        self.mcpTimeout = mcpTimeout
        self.ipVersion = ipVersion
        self.consolePort = consolePort
        self.consoleBaudRate = consoleBaudRate
        self.preloadKnowledge = preloadKnowledge
        self.preloadAnalysis = preloadAnalysis
        self.preloadRag = preloadRag
        self.preloadPlotter = preloadPlotter
        self.preloadBuilder = preloadBuilder
        self.preloadSummarization = preloadSummarization
        self.cacheExpiryMinutes = cacheExpiryMinutes
        self.nCtx = nCtx
        self.maxGen = maxGen
        self.promptKeepTokens = promptKeepTokens
        self.visionEnabled = visionEnabled
        self.autoDryRun = autoDryRun
        self.mmprojPath = mmprojPath
    }

    private enum CodingKeys: String, CodingKey {
        case historyLimit, temperature, repetitionPenalty, modelPath, recentIps, mcpTimeout, ipVersion
        case consolePort, consoleBaudRate, preloadKnowledge, preloadAnalysis, preloadRag, preloadPlotter
        case preloadBuilder, preloadSummarization, cacheExpiryMinutes, nCtx, maxGen, promptKeepTokens
        case visionEnabled, autoDryRun, mmprojPath, llmBackend
    }

    public init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        self.init(
            historyLimit: try values.decodeIfPresent(Int.self, forKey: .historyLimit) ?? 5,
            temperature: try values.decodeIfPresent(Double.self, forKey: .temperature) ?? 0.0,
            repetitionPenalty: try values.decodeIfPresent(Double.self, forKey: .repetitionPenalty) ?? 1.1,
            modelPath: try values.decodeIfPresent(String.self, forKey: .modelPath),
            recentIps: try values.decodeIfPresent([String].self, forKey: .recentIps) ?? [],
            mcpTimeout: try values.decodeIfPresent(Int.self, forKey: .mcpTimeout) ?? 30,
            ipVersion: try values.decodeIfPresent(String.self, forKey: .ipVersion) ?? "auto",
            consolePort: try values.decodeIfPresent(String.self, forKey: .consolePort),
            consoleBaudRate: try values.decodeIfPresent(Int.self, forKey: .consoleBaudRate) ?? 9600,
            preloadKnowledge: try values.decodeIfPresent(Bool.self, forKey: .preloadKnowledge) ?? false,
            preloadAnalysis: try values.decodeIfPresent(Bool.self, forKey: .preloadAnalysis) ?? false,
            preloadRag: try values.decodeIfPresent(Bool.self, forKey: .preloadRag) ?? false,
            preloadPlotter: try values.decodeIfPresent(Bool.self, forKey: .preloadPlotter) ?? false,
            preloadBuilder: try values.decodeIfPresent(Bool.self, forKey: .preloadBuilder) ?? false,
            preloadSummarization: try values.decodeIfPresent(Bool.self, forKey: .preloadSummarization) ?? false,
            cacheExpiryMinutes: try values.decodeIfPresent(Int.self, forKey: .cacheExpiryMinutes) ?? 10,
            nCtx: try values.decodeIfPresent(Int.self, forKey: .nCtx) ?? 8192,
            maxGen: try values.decodeIfPresent(Int.self, forKey: .maxGen) ?? 2048,
            promptKeepTokens: try values.decodeIfPresent(Int.self, forKey: .promptKeepTokens) ?? 500,
            visionEnabled: try values.decodeIfPresent(Bool.self, forKey: .visionEnabled) ?? false,
            autoDryRun: try values.decodeIfPresent(Bool.self, forKey: .autoDryRun) ?? false,
            mmprojPath: try values.decodeIfPresent(String.self, forKey: .mmprojPath),
            llmBackend: try values.decodeIfPresent(LLMBackend.self, forKey: .llmBackend) ?? .llamacpp
        )
    }

    public mutating func merge(_ patch: DesktopSettingsPatch) {
        apply(patch.historyLimit, to: \.historyLimit)
        apply(patch.temperature, to: \.temperature)
        apply(patch.repetitionPenalty, to: \.repetitionPenalty)
        apply(patch.modelPath, to: \.modelPath)
        apply(patch.llmBackend, to: \.llmBackend)
        apply(patch.recentIps, to: \.recentIps)
        apply(patch.mcpTimeout, to: \.mcpTimeout)
        apply(patch.ipVersion, to: \.ipVersion)
        apply(patch.consolePort, to: \.consolePort)
        apply(patch.consoleBaudRate, to: \.consoleBaudRate)
        apply(patch.preloadKnowledge, to: \.preloadKnowledge)
        apply(patch.preloadAnalysis, to: \.preloadAnalysis)
        apply(patch.preloadRag, to: \.preloadRag)
        apply(patch.preloadPlotter, to: \.preloadPlotter)
        apply(patch.preloadBuilder, to: \.preloadBuilder)
        apply(patch.preloadSummarization, to: \.preloadSummarization)
        apply(patch.cacheExpiryMinutes, to: \.cacheExpiryMinutes)
        apply(patch.nCtx, to: \.nCtx)
        apply(patch.maxGen, to: \.maxGen)
        apply(patch.promptKeepTokens, to: \.promptKeepTokens)
        apply(patch.visionEnabled, to: \.visionEnabled)
        apply(patch.autoDryRun, to: \.autoDryRun)
        apply(patch.mmprojPath, to: \.mmprojPath)
    }

    private mutating func apply<Value>(_ update: SettingUpdate<Value>, to keyPath: WritableKeyPath<DesktopSettings, Value>) {
        if case let .set(value) = update { self[keyPath: keyPath] = value }
    }
}

public enum SettingUpdate<Value> {
    case unchanged
    case set(Value)
}

public struct DesktopSettingsPatch {
    public var historyLimit: SettingUpdate<Int> = .unchanged
    public var temperature: SettingUpdate<Double> = .unchanged
    public var repetitionPenalty: SettingUpdate<Double> = .unchanged
    public var modelPath: SettingUpdate<String?> = .unchanged
    public var llmBackend: SettingUpdate<LLMBackend> = .unchanged
    public var recentIps: SettingUpdate<[String]> = .unchanged
    public var mcpTimeout: SettingUpdate<Int?> = .unchanged
    public var ipVersion: SettingUpdate<String?> = .unchanged
    public var consolePort: SettingUpdate<String?> = .unchanged
    public var consoleBaudRate: SettingUpdate<Int?> = .unchanged
    public var preloadKnowledge: SettingUpdate<Bool> = .unchanged
    public var preloadAnalysis: SettingUpdate<Bool> = .unchanged
    public var preloadRag: SettingUpdate<Bool> = .unchanged
    public var preloadPlotter: SettingUpdate<Bool> = .unchanged
    public var preloadBuilder: SettingUpdate<Bool> = .unchanged
    public var preloadSummarization: SettingUpdate<Bool> = .unchanged
    public var cacheExpiryMinutes: SettingUpdate<Int?> = .unchanged
    public var nCtx: SettingUpdate<Int> = .unchanged
    public var maxGen: SettingUpdate<Int> = .unchanged
    public var promptKeepTokens: SettingUpdate<Int> = .unchanged
    public var visionEnabled: SettingUpdate<Bool> = .unchanged
    public var autoDryRun: SettingUpdate<Bool> = .unchanged
    public var mmprojPath: SettingUpdate<String?> = .unchanged

    public init(temperature: SettingUpdate<Double> = .unchanged) {
        self.temperature = temperature
    }
}

public enum DesktopSettingsCodec {
    public static func decode(_ data: Data) throws -> DesktopSettings {
        try JSONDecoder().decode(DesktopSettings.self, from: data)
    }

    public static func encode(_ settings: DesktopSettings) throws -> Data {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        return try encoder.encode(settings)
    }
}
