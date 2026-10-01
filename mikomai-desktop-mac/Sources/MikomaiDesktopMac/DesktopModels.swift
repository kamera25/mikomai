import SwiftUI
import AppKit
import Foundation
import Darwin
import Security
import CryptoKit
import MikomaiFFI
import MikomaiDesktopCore
import UniformTypeIdentifiers

// MARK: - Enums & Models

enum Workspace: String, CaseIterable, Identifiable {
    case chat = "チャット"
    case connections = "機器情報一覧"
    case tools = "ネットワークツール"
    case monitoring = "監視・タスク履歴"
    case settings = "設定"

    var id: String { rawValue }
    var icon: String {
        switch self {
        case .chat: "bubble.left.and.bubble.right"
        case .connections: "point.3.connected.trianglepath.dotted"
        case .tools: "wrench.and.screwdriver"
        case .monitoring: "waveform.path.ecg"
        case .settings: "gearshape"
        }
    }
}

enum ToolTab: String, CaseIterable, Identifiable {
    case tcpTest = "接続テスト"
    case ping = "Ping / Trace"
    case arp = "ARP テーブル"
    case route = "ルーティング"

    var id: String { rawValue }
    var icon: String {
        switch self {
        case .tcpTest: "bolt.horizontal.fill"
        case .ping: "antenna.radiowaves.left.and.right"
        case .arp: "tablecells"
        case .route: "arrow.triangle.branch"
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
        if let env = ProcessInfo.processInfo.environment["MIKOMAI_SETTINGS_PATH"], !env.isEmpty {
            return URL(fileURLWithPath: env)
        }
        let support = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first
            ?? FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Library/Application Support")
        return support.appendingPathComponent("MikomaiDesktopMac/settings.json")
    }

    static func load() -> (settings: AppSettings, url: URL, source: String?) {
        let url = settingsURL
        if let data = try? Data(contentsOf: url), let decoded = try? DesktopSettingsCodec.decode(data) {
            return (decoded, url, "native")
        }

        // Import existing installs once; all future saves go to the native app store.
        let support = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first
        let legacyURLs = [
            support?.appendingPathComponent("com.mikomai.agent/settings.json"),
            support?.appendingPathComponent("mikomai/settings.json"),
            FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".config/mikomai/settings.json")
        ].compactMap { $0 }.filter { $0.standardizedFileURL != url.standardizedFileURL }
        for legacyURL in legacyURLs {
            guard let data = try? Data(contentsOf: legacyURL),
                  let decoded = try? DesktopSettingsCodec.decode(data) else { continue }
            try? save(decoded)
            return (decoded, url, "imported")
        }
        return (AppSettings(), url, nil)
    }

    static func save(_ settings: AppSettings) throws {
        let url = settingsURL
        let dir = url.deletingLastPathComponent()
        if !FileManager.default.fileExists(atPath: dir.path) {
            try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        }
        let data = try DesktopSettingsCodec.encode(settings)
        let tempURL = url.appendingPathExtension("tmp")
        try data.write(to: tempURL, options: .atomic)
        if FileManager.default.fileExists(atPath: url.path) {
            _ = try? FileManager.default.removeItem(at: url)
        }
        try FileManager.default.moveItem(at: tempURL, to: url)
    }
}

// MARK: - Chat & Saved Connections Models

struct ConnectionTestStatus {
    let success: Bool
    let message: String
    let latencyMs: Int?
    let timestamp: Date
}

struct ArpRecord: Identifiable {
    let id = UUID()
    let ip: String
    let mac: String
    let interface: String
    let isPermanent: Bool
    let isIncomplete: Bool
}

struct RouteRecord: Identifiable {
    let id = UUID()
    let destination: String
    let gateway: String
    let flags: String
    let interface: String
}

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

    init(_ connection: SavedConnection, credentials: ConnectionCredentials) {
        id = connection.id.uuidString
        name = connection.name
        host = connection.host
        username = connection.username
        deviceType = connection.deviceType
        connectionType = connection.connectionType ?? "SSH"
        port = connection.port
        let credentialText = "\(credentials.password ?? "")\u{0}\(credentials.enablePassword ?? "")"
        credentialsFingerprint = SHA256.hash(data: Data(credentialText.utf8)).map { String(format: "%02x", $0) }.joined()
    }
}

struct NetworkRunnerRequest: Sendable {
    let action: String
    let host: String
    let username: String
    let password: String
    let secret: String
    let deviceType: String
    let port: String
    let commands: [String]
}

struct NetworkOperationOutput: Sendable {
    let success: Bool
    let stdout: String
    let stderr: String
}

// MARK: - Keychain Helper

private enum KeychainHelper {
    private static let service = "com.mikomai.desktop.mac"

    static func save(key: String, value: String) {
        guard let data = value.data(using: .utf8) else { return }
        delete(key: key)
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: key,
            kSecValueData as String: data,
            kSecAttrAccessible as String: kSecAttrAccessibleAfterFirstUnlock
        ]
        SecItemAdd(query as CFDictionary, nil)
    }

    static func load(key: String) -> String? {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: key,
            kSecReturnData as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne
        ]
        var result: AnyObject?
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        guard status == errSecSuccess, let data = result as? Data else { return nil }
        return String(data: data, encoding: .utf8)
    }

    static func delete(key: String) {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: key
        ]
        SecItemDelete(query as CFDictionary)
    }
}

struct KeychainCredentialAdapter: CredentialStore {
    func save(key: String, value: String) { KeychainHelper.save(key: key, value: value) }
    func load(key: String) -> String? { KeychainHelper.load(key: key) }
    func delete(key: String) { KeychainHelper.delete(key: key) }
}
