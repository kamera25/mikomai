import SwiftUI
import AppKit
import Foundation
import Darwin
import Security
import CryptoKit
import MikomaiFFI
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
    let credentialPersistence: ConnectionCredentialPersistence
    let onOperationPlan: (Data) -> Void
    let onToolResult: (String, String, Bool) -> Void

    init(stream: StreamBox, connections: [SavedConnection], credentialPersistence: ConnectionCredentialPersistence, onOperationPlan: @escaping (Data) -> Void, onToolResult: @escaping (String, String, Bool) -> Void) {
        self.stream = stream
        self.connections = connections
        self.credentialPersistence = credentialPersistence
        self.onOperationPlan = onOperationPlan
        self.onToolResult = onToolResult
    }
}

func streamBridge(chunk: UnsafePointer<CChar>?, isDone: Int32, context: UnsafeMutableRawPointer?) {
    guard let context else { return }
    let box = Unmanaged<ChatCallbackBox>.fromOpaque(context).takeUnretainedValue()
    let text = chunk.flatMap { String(cString: $0) } ?? ""
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
    let credentialPersistence: ConnectionCredentialPersistence
    let onNotification: @Sendable (Data) -> Void
    init(connections: [SavedConnection], credentialPersistence: ConnectionCredentialPersistence, onNotification: @escaping @Sendable (Data) -> Void) {
        self.savedConnections = connections
        self.credentialPersistence = credentialPersistence
        self.onNotification = onNotification
    }
    var connections: [SavedConnection] { lock.lock(); defer { lock.unlock() }; return savedConnections }
    func update(connections: [SavedConnection]) { lock.lock(); savedConnections = connections; lock.unlock() }
}

func watchToolBridge(
    toolID: UnsafePointer<CChar>?, targetJSON: UnsafePointer<CChar>?, argsJSON: UnsafePointer<CChar>?,
    output: UnsafeMutablePointer<CChar>?, outputCapacity: UInt, context: UnsafeMutableRawPointer?
) -> Int32 {
    guard let output, outputCapacity > 0 else { return 1 }
    let capacity = Int(outputCapacity); output[0] = 0
    guard let context, let toolID, let targetJSON, let argsJSON else { return 1 }
    let box = Unmanaged<WatchCallbackBox>.fromOpaque(context).takeUnretainedValue()
    do {
        let target = try JSONDecoder().decode(PortableDeviceTarget.self, from: Data(String(cString: targetJSON).utf8))
        let arguments = try JSONSerialization.jsonObject(with: Data(String(cString: argsJSON).utf8)) as? [String: Any] ?? [:]
        let result = DesktopModel.runPortableAgentTool(tool: String(cString: toolID), target: target, arguments: arguments, connections: box.connections, credentialPersistence: box.credentialPersistence)
        let payload = try JSONSerialization.data(withJSONObject: ["success": result.success, "output": result.success ? result.stdout : result.stderr])
        let text = String(decoding: payload, as: UTF8.self)
        return text.withCString { strlcpy(output, $0, capacity) < capacity ? 0 : 1 }
    } catch {
        let text = "watch probe failed: \(error.localizedDescription)"
        return text.withCString { _ = strlcpy(output, $0, capacity); return 1 }
    }
}

func watchNotificationBridge(notificationJSON: UnsafePointer<CChar>?, context: UnsafeMutableRawPointer?) {
    guard let context, let notificationJSON else { return }
    let box = Unmanaged<WatchCallbackBox>.fromOpaque(context).takeUnretainedValue()
    box.onNotification(Data(String(cString: notificationJSON).utf8))
}

func agentToolBridge(
    toolID: UnsafePointer<CChar>?,
    targetJSON: UnsafePointer<CChar>?,
    argsJSON: UnsafePointer<CChar>?,
    output: UnsafeMutablePointer<CChar>?,
    outputCapacity: UInt,
    context: UnsafeMutableRawPointer?
) -> Int32 {
    guard let output, outputCapacity > 0 else { return 1 }
    let capacity = Int(outputCapacity)
    output[0] = 0
    guard let context, let toolID, let targetJSON, let argsJSON else {
        "agent tool bridge arguments are missing".withCString { _ = strlcpy(output, $0, capacity) }
        return 1
    }
    let box = Unmanaged<ChatCallbackBox>.fromOpaque(context).takeUnretainedValue()
    let tool = String(cString: toolID)
    do {
        let target = try JSONDecoder().decode(PortableDeviceTarget.self, from: Data(String(cString: targetJSON).utf8))
        let arguments = try JSONSerialization.jsonObject(with: Data(String(cString: argsJSON).utf8)) as? [String: Any] ?? [:]
        let result = DesktopModel.runPortableAgentTool(
            tool: tool,
            target: target,
            arguments: arguments,
            connections: box.connections,
            credentialPersistence: box.credentialPersistence
        )
        if tool == "get_state" || tool == "query_db" {
            box.onToolResult(tool, result.success ? result.stdout : result.stderr, result.success)
        }
        let payload = try JSONSerialization.data(withJSONObject: ["success": result.success, "output": result.success ? result.stdout : result.stderr])
        let text = String(decoding: payload, as: UTF8.self)
        let copied = text.withCString { strlcpy(output, $0, capacity) }
        return copied < capacity ? 0 : 1
    } catch {
        let text = "agent tool failed: \(error.localizedDescription)"
        let _ = text.withCString { strlcpy(output, $0, capacity) }
        return 1
    }
}

func agentPlanBridge(
    target: UnsafePointer<CChar>?,
    toolID: UnsafePointer<CChar>?,
    argsJSON: UnsafePointer<CChar>?,
    rationale: UnsafePointer<CChar>?,
    output: UnsafeMutablePointer<CChar>?,
    outputCapacity: UInt,
    context: UnsafeMutableRawPointer?
) -> Int32 {
    guard let output, outputCapacity > 0 else { return 1 }
    let capacity = Int(outputCapacity)
    output[0] = 0
    guard let context, let target, let toolID, let argsJSON, let rationale else { return 1 }
    let box = Unmanaged<ChatCallbackBox>.fromOpaque(context).takeUnretainedValue()
    let targetName = String(cString: target)
    guard let connection = box.connections.first(where: { $0.name == targetName || $0.host == targetName || $0.id.uuidString == targetName }) else {
        "変更対象がSwift側の登録端末にありません。".withCString { _ = strlcpy(output, $0, capacity) }
        return 1
    }
    guard let credentialsJSON = try? String(data: JSONEncoder().encode(NativeDeviceSnapshot(connection, credentials: box.credentialPersistence.load(for: connection.id))), encoding: .utf8) else { return 1 }
    let toolName = String(cString: toolID)
    let args = String(cString: argsJSON)
    let rationaleText = String(cString: rationale)
    let response = connection.name.withCString { targetPtr in
        credentialsJSON.withCString { snapshotPtr in
            toolName.withCString { toolPtr in
                args.withCString { argsPtr in
                rationaleText.withCString { rationalePtr in
                    mikomai_operation_plan_create_generic(targetPtr, toolPtr, snapshotPtr, argsPtr, rationalePtr)
                }
                }
            }
        }
    }
    defer { mikomai_result_free(response) }
    guard response.status == 0, let message = response.message else {
        let text = response.message.map { String(cString: $0) } ?? "変更計画を作成できませんでした。"
        let _ = text.withCString { strlcpy(output, $0, capacity) }
        return 1
    }
    let copied = strlcpy(output, message, capacity)
    return copied < capacity ? 0 : 1
}
