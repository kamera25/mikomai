import SwiftUI
import AppKit
import Foundation
import Darwin
import Security
import CryptoKit
import MikomaiBindings
import MikomaiDesktopCore
import UniformTypeIdentifiers

// MARK: - C ABI Streaming Callback Bridge

final class StreamBox: @unchecked Sendable {
    let onChunk: (String, Bool) -> Void
    init(onChunk: @escaping (String, Bool) -> Void) {
        self.onChunk = onChunk
    }
}

final class ChatCallbackBox: @unchecked Sendable {
    let stream: StreamBox
    let connections: [SavedConnection]
    let onOperationPlan: (Data) -> Void
    let onToolResult: (AgentToolResult) -> Void
    let onDebug: (String) -> Void

    init(stream: StreamBox, connections: [SavedConnection], onOperationPlan: @escaping (Data) -> Void, onToolResult: @escaping (AgentToolResult) -> Void, onDebug: @escaping (String) -> Void) {
        self.stream = stream
        self.connections = connections
        self.onOperationPlan = onOperationPlan
        self.onToolResult = onToolResult
        self.onDebug = onDebug
    }
}

func streamBridge(chunk: UnsafePointer<CChar>?, isDone: Int32, context: UnsafeMutableRawPointer?) {
    guard let context else { return }
    let box = Unmanaged<ChatCallbackBox>.fromOpaque(context).takeUnretainedValue()
    let text = chunk.flatMap { String(cString: $0) } ?? ""
    if text.hasPrefix("__MIKOMAI_DEBUG__") {
        box.onDebug(String(text.dropFirst("__MIKOMAI_DEBUG__".count)))
        return
    }
    box.onDebug(CoreDebugRecord.encode(kind: "core_stream", payload: ["text": text, "done": isDone != 0]))
    let approvalPrefix = "__MIKOMAI_APPROVAL_PLAN__"
    if text.hasPrefix(approvalPrefix) {
        let json = String(text.dropFirst(approvalPrefix.count)).components(separatedBy: "\n").first ?? ""
        box.onOperationPlan(Data(json.utf8))
        return
    }
    box.stream.onChunk(text, isDone != 0)
}

struct PortableDeviceTarget: Decodable {
    let id: String?
    let hostname: String
    let ip: String?
    let deviceType: String?
}

struct NativeWatch: Codable, Identifiable {
    struct IR: Codable {
        struct Schedule: Codable { var every: String }
        struct CallArgs: Codable { var device: String; var resource: String }
        struct Call: Codable { var id: String; var call: String; var args: CallArgs }
        struct Reference: Codable { var ref: String }
        struct Comparison: Codable { var left: Reference; var `operator`: String; var right: Double }
        struct NotificationArgs: Codable { var message: String }
        struct Notification: Codable { var call: String; var args: NotificationArgs }
        struct When: Codable { var when: Comparison; var then: [Notification] }
        var version: Int
        var schedule: Schedule
        var steps: [Step]
        enum Step: Codable {
            case call(Call)
            case when(When)
            init(from decoder: Decoder) throws {
                let container = try decoder.singleValueContainer()
                if let call = try? container.decode(Call.self) { self = .call(call); return }
                self = .when(try container.decode(When.self))
            }
            func encode(to encoder: Encoder) throws {
                var container = encoder.singleValueContainer()
                switch self { case .call(let value): try container.encode(value); case .when(let value): try container.encode(value) }
            }
        }
    }
    struct Run: Codable, Identifiable {
        struct Notice: Codable, Identifiable { var watchId: String; var message: String; var emittedAt: String; var id: String { emittedAt } }
        var runId: String; var startedAt: String; var completedAt: String; var notifications: [Notice]; var error: String?
        var id: String { runId }
    }
    var id: String
    var name: String
    var status: String
    var ir: IR
    var createdAt: String
    var lastRunAt: String?
    var lastError: String?
    var history: [Run]?
}

struct WatchAlert: Identifiable {
    let id = UUID()
    let message: String
}

final class WatchCallbackBox: @unchecked Sendable {
    private let lock = NSLock()
    private var savedConnections: [SavedConnection]
    let onNotification: @Sendable (Data) -> Void
    init(connections: [SavedConnection], onNotification: @escaping @Sendable (Data) -> Void) {
        self.savedConnections = connections
        self.onNotification = onNotification
    }
    var connections: [SavedConnection] { lock.lock(); defer { lock.unlock() }; return savedConnections }
    func update(connections: [SavedConnection]) { lock.lock(); savedConnections = connections; lock.unlock() }
}

func watchNotificationBridge(notificationJSON: UnsafePointer<CChar>?, context: UnsafeMutableRawPointer?) {
    guard let context, let notificationJSON else { return }
    let box = Unmanaged<WatchCallbackBox>.fromOpaque(context).takeUnretainedValue()
    box.onNotification(Data(String(cString: notificationJSON).utf8))
}

