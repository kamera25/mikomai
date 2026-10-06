import SwiftUI
import AppKit
import Foundation
import Darwin
import MikomaiBindings
import MikomaiDesktopCore
import UniformTypeIdentifiers

// MARK: - Enums & Models

enum Workspace: String, CaseIterable, Identifiable {
    case chat = "チャット"
    case connections = "機器情報一覧"
    case agentHistory = "エージェント履歴"
    case monitoring = "CPU監視"
    case settings = "設定"

    var id: String { rawValue }
    var icon: String {
        switch self {
        case .chat: "bubble.left.and.bubble.right"
        case .connections: "point.3.connected.trianglepath.dotted"
        case .agentHistory: "clock.arrow.circlepath"
        case .monitoring: "waveform.path.ecg"
        case .settings: "gearshape"
        }
    }
}

// MARK: - Native Settings Model

typealias AppSettings = DesktopSettings

// MARK: - Model Presets

let PRESET_MODELS = ModelPresetCatalog.presets

// MARK: - Hugging Face Hub Helper

enum HuggingFaceHub {
    static var cacheDirectory: URL {
        if let env = ProcessInfo.processInfo.environment["HF_HUB_CACHE"], !env.isEmpty {
            return URL(fileURLWithPath: env)
        }
        if let envHome = ProcessInfo.processInfo.environment["HF_HOME"], !envHome.isEmpty {
            return URL(fileURLWithPath: envHome).appendingPathComponent("hub")
        }
        return FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".cache/huggingface/hub")
    }

    static func modelURL(repo: String, filename: String) -> URL {
        cacheDirectory.appendingPathComponent(repo).appendingPathComponent(filename)
    }

    static func modelExists(repo: String, filename: String) -> Bool {
        let url = modelURL(repo: repo, filename: filename)
        return FileManager.default.fileExists(atPath: url.path)
    }
}

// MARK: - Serial Port Helper

enum SerialPortDetector {
    static func listPorts() -> [String] {
        let fm = FileManager.default
        guard let files = try? fm.contentsOfDirectory(atPath: "/dev") else { return [] }
        return files
            .filter { $0.hasPrefix("cu.") || $0.hasPrefix("tty.") }
            .map { "/dev/\($0)" }
            .sorted()
    }
}

// MARK: - Native Settings Store

enum SettingsManager {
    static var settingsURL: URL {
        if let path = ProcessInfo.processInfo.environment["MIKOMAI_GRAPH_DB_PATH"] { return URL(fileURLWithPath: path) }
        return FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Library/Application Support/MikomaiDesktopMac/surrealdb")
    }

    static func load() throws -> (settings: AppSettings, url: URL, source: String?) {
        let decoded: AppSettings? = try NativePersistence.load("settings")
        return (decoded ?? AppSettings(), settingsURL, decoded == nil ? nil : "native")
    }
    static func save(_ settings: AppSettings) throws {
        try NativePersistence.save(settings, collection: "settings")
    }

}

// MARK: - Chat & Saved Connections Models

struct NativeOperationPlan: Decodable, Identifiable {
    let id: String
    let planHash: String
    let status: String
    let toolId: String
    let rationale: String
    var target: String?
    let args: NativeOperationPlanArgs
}

struct NativeOperationPlanArgs: Decodable {
    let commands: [String]?
    let deviceSnapshot: NativeDeviceSnapshot
}

struct NativeDeviceSnapshot: Codable, Equatable {
    let id: String
    let name: String
    let host: String
    let username: String
    let deviceType: String
    let connectionType: String
    let port: String
    let credentialsFingerprint: String

}

struct NetworkOperationOutput: Sendable {
    let success: Bool
    let stdout: String
    let stderr: String
    var command: String? = nil
    var exitCode: Int32? = nil
}

