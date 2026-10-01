import Foundation

/// Agent task list row sent by the native FFI layer.
public struct NativeAgentTask: Decodable, Identifiable, Equatable, Sendable {
    public var taskId: String
    public var goal: String
    public var status: String
    public var startedAt: String
    public var lastEventAt: String
    public var eventCount: Int
    public var id: String { taskId }

    public init(taskId: String, goal: String, status: String, startedAt: String, lastEventAt: String, eventCount: Int) {
        self.taskId = taskId
        self.goal = goal
        self.status = status
        self.startedAt = startedAt
        self.lastEventAt = lastEventAt
        self.eventCount = eventCount
    }
}

/// A bounded result shown in the conversation sidebar after a native tool call.
public struct AgentToolResult: Identifiable, Equatable, Sendable {
    public var id: UUID
    public var tool: String
    public var output: String
    public var succeeded: Bool
    public var command: String
    public var sessionID: UUID?
    public var isLocalProbe: Bool { Self.isLocalProbe(tool: tool) }
    public static func isLocalProbe(tool: String) -> Bool {
        ["self_network_ping", "self_network_traceroute", "self_ping", "self_trace"].contains(tool)
    }
    public static func terminalOutput(stdout: String, stderr: String) -> String {
        [stdout, stderr].filter { !$0.isEmpty }.joined(separator: stdout.hasSuffix("\n") ? "" : "\n")
    }

    public init(id: UUID = UUID(), tool: String, output: String, succeeded: Bool, command: String? = nil, sessionID: UUID? = nil) {
        self.id = id
        self.tool = tool
        self.output = output
        self.succeeded = succeeded
        self.command = command ?? tool
        self.sessionID = sessionID
    }
}

/// Converts persisted Rust Agent events into short, readable progress entries.
public enum AgentTaskHistoryPresentation {
    public static func lines(from json: String, fallbackGoal: String) -> [String] {
        guard let data = json.data(using: .utf8),
              let root = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let events = root["events"] as? [[String: Any]] else {
            return json.isEmpty ? [] : [json]
        }

        return events.compactMap { event in
            switch event["event_type"] as? String {
            case "task_started":
                return "開始: \(event["goal"] as? String ?? fallbackGoal)"
            case "observation":
                let evidence = event["evidence"] as? [String: Any] ?? [:]
                let output = evidence["content"] as? String
                    ?? evidence["raw"] as? String
                    ?? Self.prettyJSON(evidence.isEmpty ? event : evidence)
                return "観測結果:\n\(output)"
            case "state_updated":
                return "状態: \(Self.stringValue(event["status"]) ?? "更新中")"
            case "approval_required":
                return "承認待ち: \(event["message"] as? String ?? "提案を確認してください")"
            case "finished":
                return "完了:\n\(event["answer"] as? String ?? "回答を記録しました")"
            default:
                return nil
            }
        }
    }

    private static func stringValue(_ value: Any?) -> String? {
        guard let value else { return nil }
        if let text = value as? String { return text }
        return prettyJSON(value)
    }

    private static func prettyJSON(_ value: Any) -> String {
        guard JSONSerialization.isValidJSONObject(value),
              let data = try? JSONSerialization.data(withJSONObject: value, options: [.prettyPrinted, .sortedKeys]),
              let text = String(data: data, encoding: .utf8) else { return "観測結果を記録しました" }
        return text
    }
}

/// Structured progress is kept apart from the final answer and persisted per message.
public struct AgentProgressEntry: Codable, Equatable, Identifiable {
    public var id = UUID()
    public var phase: String
    public var nextAction: String
    public var detail: String

    public init(phase: String, nextAction: String, detail: String) {
        self.phase = phase
        self.nextAction = nextAction
        self.detail = detail
    }

    public static let streamPrefix = "__MIKOMAI_AGENT_PROGRESS__"
    public static func parse(_ chunk: String) -> Self? {
        guard chunk.hasPrefix(streamPrefix),
              let data = String(chunk.dropFirst(streamPrefix.count)).data(using: .utf8),
              let value = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let phase = value["phase"] as? String,
              let next = value["nextAction"] as? String,
              let detail = value["detail"] as? String else { return nil }
        return Self(phase: phase, nextAction: next, detail: detail)
    }
}
