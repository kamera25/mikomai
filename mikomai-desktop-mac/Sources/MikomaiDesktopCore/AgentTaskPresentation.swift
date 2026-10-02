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
    public static func taskDateGroups(
        _ tasks: [NativeAgentTask],
        calendar: Calendar = .current
    ) -> [AgentTaskDateGroup] {
        var groups: [AgentTaskDateGroup] = []
        for task in tasks {
            if let date = parseTimestamp(task.lastEventAt) {
                let day = calendar.startOfDay(for: date)
                if let lastIndex = groups.indices.last,
                   let lastDate = groups[lastIndex].date,
                   calendar.isDate(lastDate, inSameDayAs: date) {
                    groups[lastIndex].tasks.append(task)
                } else {
                    let components = calendar.dateComponents([.year, .month, .day], from: day)
                    let dateID = "\(components.year ?? 0)-\(components.month ?? 0)-\(components.day ?? 0)"
                    groups.append(AgentTaskDateGroup(id: "\(dateID)-\(groups.count)", date: day, tasks: [task]))
                }
            } else if let lastIndex = groups.indices.last, groups[lastIndex].date == nil {
                groups[lastIndex].tasks.append(task)
            } else {
                groups.append(AgentTaskDateGroup(id: "unknown-date-\(groups.count)", date: nil, tasks: [task]))
            }
        }
        return groups
    }

    public static func items(from json: String, fallbackGoal: String) -> [AgentTaskHistoryItem] {
        guard let data = json.data(using: .utf8),
              let root = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let events = root["events"] as? [[String: Any]] else {
            return json.isEmpty ? [] : [AgentTaskHistoryItem(
                id: 0,
                eventType: "fallback",
                timestamp: nil,
                icon: .unknown,
                title: "記録",
                detail: json
            )]
        }

        return events.enumerated().compactMap { index, event in
            guard let eventType = event["event_type"] as? String else { return nil }
            let timestamp = timestamp(in: event)
            switch eventType {
            case "task_started":
                let goal = event["goal"] as? String ?? fallbackGoal
                return AgentTaskHistoryItem(
                    id: index,
                    eventType: eventType,
                    timestamp: timestamp,
                    icon: .started,
                    title: "開始",
                    detail: goal
                )
            case "goal_set":
                let goal = event["goal"] as? String ?? fallbackGoal
                return AgentTaskHistoryItem(
                    id: index,
                    eventType: eventType,
                    timestamp: timestamp,
                    icon: .started,
                    title: "目標設定",
                    detail: goal
                )
            case "decision":
                let actionType = event["action_type"] as? String
                let title = actionType.map { "判断: \($0)" } ?? "判断"
                var details: [String] = []
                if let objective = event["objective"] as? String, !objective.isEmpty {
                    details.append(objective)
                }
                if let reason = event["reason"] as? [String], !reason.isEmpty {
                    details.append(reason.joined(separator: "\n"))
                } else if let reasonStr = event["reason"] as? String, !reasonStr.isEmpty {
                    details.append(reasonStr)
                }
                if let params = event["parameters"] {
                    details.append(prettyJSON(params))
                }
                let detail = details.isEmpty ? (event["goal"] as? String ?? "判断を実行") : details.joined(separator: "\n")
                return AgentTaskHistoryItem(
                    id: index,
                    eventType: eventType,
                    timestamp: timestamp,
                    icon: .running,
                    title: title,
                    detail: detail
                )
            case "action":
                let tool = event["tool"] as? String ?? ""
                let title = tool.isEmpty ? "実行" : "実行: \(tool)"
                let target = event["target"] as? String
                var parts: [String] = []
                if let target, !target.isEmpty { parts.append(target) }
                if !tool.isEmpty { parts.append(tool) }
                if let params = event["parameters"] {
                    parts.append(prettyJSON(params))
                }
                let detail = parts.isEmpty ? "ツールを実行" : parts.joined(separator: " / ")
                return AgentTaskHistoryItem(
                    id: index,
                    eventType: eventType,
                    timestamp: timestamp,
                    icon: .running,
                    title: title,
                    detail: detail
                )
            case "result":
                let observation = event["observation"] as? [String: Any]
                let source = observation?["source"] as? [String: Any]
                let tool = source?["tool_name"] as? String ?? event["tool"] as? String ?? "ツール"
                let success = event["success"] as? Bool
                let error = stringValue(event["error"])
                let title: String
                if let error, !error.isEmpty {
                    title = "失敗 (\(error)): \(tool)"
                } else if success == false {
                    title = "失敗: \(tool)"
                } else {
                    title = "成功: \(tool)"
                }
                let raw = observation?["raw"] as? String
                    ?? observation?["content"] as? String
                    ?? error
                    ?? (observation.map(prettyJSON) ?? prettyJSON(event))
                return AgentTaskHistoryItem(
                    id: index,
                    eventType: eventType,
                    timestamp: timestamp,
                    icon: (error != nil || success == false) ? .failed : .completed,
                    title: title,
                    detail: raw
                )
            case "observation":
                let evidence = event["evidence"] as? [String: Any] ?? [:]
                let output = evidence["content"] as? String
                    ?? evidence["raw"] as? String
                    ?? event["raw"] as? String
                    ?? prettyJSON(evidence.isEmpty ? event : evidence)
                return AgentTaskHistoryItem(
                    id: index,
                    eventType: eventType,
                    timestamp: timestamp,
                    icon: .observation,
                    title: "観測結果",
                    detail: output
                )
            case "state_updated":
                let statusRaw = stringValue(event["status"]) ?? "更新中"
                var displayTitle = "状態"
                var displayDetail = statusRaw
                var rawStatus = statusRaw
                if let statusData = statusRaw.data(using: .utf8),
                   let statusObj = try? JSONSerialization.jsonObject(with: statusData) as? [String: Any] {
                    if let phase = statusObj["phase"] as? String {
                        displayTitle = "状態 (\(phase))"
                    }
                    var parts: [String] = []
                    if let d = statusObj["detail"] as? String, !d.isEmpty { parts.append(d) }
                    if let n = statusObj["nextAction"] as? String, !n.isEmpty { parts.append("次: \(n)") }
                    if !parts.isEmpty {
                        displayDetail = parts.joined(separator: "\n")
                    }
                    rawStatus = statusObj["status"] as? String ?? statusRaw
                }
                return AgentTaskHistoryItem(
                    id: index,
                    eventType: eventType,
                    timestamp: timestamp,
                    icon: AgentTaskHistoryIcon(status: rawStatus),
                    title: displayTitle,
                    detail: displayDetail,
                    rawStatus: rawStatus
                )
            case "approval_required":
                return AgentTaskHistoryItem(
                    id: index,
                    eventType: eventType,
                    timestamp: timestamp,
                    icon: .awaitingApproval,
                    title: "承認待ち",
                    detail: event["message"] as? String ?? "提案を確認してください"
                )
            case "finished":
                if let error = stringValue(event["error"]), !error.isEmpty {
                    return AgentTaskHistoryItem(
                        id: index,
                        eventType: eventType,
                        timestamp: timestamp,
                        icon: .failed,
                        title: "失敗",
                        detail: error
                    )
                }
                let answer = event["answer"] as? String ?? event["reason"] as? String ?? "回答を記録しました"
                return AgentTaskHistoryItem(
                    id: index,
                    eventType: eventType,
                    timestamp: timestamp,
                    icon: .completed,
                    title: "完了",
                    detail: answer
                )
            default:
                let detail = event["detail"] as? String ?? event["reason"] as? String ?? prettyJSON(event)
                return AgentTaskHistoryItem(
                    id: index,
                    eventType: eventType,
                    timestamp: timestamp,
                    icon: .unknown,
                    title: eventType,
                    detail: detail
                )
            }
        }
    }

    /// Preserves the text presentation used by older callers and history fixtures.
    public static func lines(from json: String, fallbackGoal: String) -> [String] {
        items(from: json, fallbackGoal: fallbackGoal).map { item in
            switch item.eventType {
            case "fallback": return item.detail
            case "task_started": return "開始: \(item.detail)"
            case "observation": return "観測結果:\n\(item.detail)"
            case "state_updated": return "状態: \(item.rawStatus ?? item.detail)"
            case "approval_required": return "承認待ち: \(item.detail)"
            case "finished":
                return item.icon == .failed ? "失敗:\n\(item.detail)" : "完了:\n\(item.detail)"
            default: return "\(item.title): \(item.detail)"
            }
        }
    }

    public static func startsNewDay(
        at index: Int,
        in items: [AgentTaskHistoryItem],
        calendar: Calendar = .current
    ) -> Bool {
        guard items.indices.contains(index), let date = items[index].timestamp else { return false }
        guard let previous = items.prefix(index).reversed().compactMap(\.timestamp).first else { return true }
        return !calendar.isDate(previous, inSameDayAs: date)
    }

    public static func timeLabel(
        _ date: Date,
        calendar: Calendar = .current,
        locale: Locale = .current
    ) -> String {
        let formatter = DateFormatter()
        formatter.locale = locale
        formatter.calendar = calendar
        formatter.timeZone = calendar.timeZone
        formatter.dateFormat = "HH:mm"
        return formatter.string(from: date)
    }

    public static func taskUpdateTimeLabel(
        _ timestamp: String?,
        calendar: Calendar = .current,
        locale: Locale = .current
    ) -> String {
        guard let timestamp, let date = parseTimestamp(timestamp) else { return "時刻不明" }
        return timeLabel(date, calendar: calendar, locale: locale)
    }

    public static func dateLabel(
        _ date: Date,
        now: Date = Date(),
        calendar: Calendar = .current,
        locale: Locale = .current
    ) -> String {
        let formatter = DateFormatter()
        formatter.locale = locale
        formatter.calendar = calendar
        formatter.timeZone = calendar.timeZone
        let dateYear = calendar.component(.year, from: date)
        let currentYear = calendar.component(.year, from: now)
        formatter.dateFormat = dateYear < currentYear ? "yyyy年M月d日" : "M月d日"
        return formatter.string(from: date)
    }

    private static func timestamp(in event: [String: Any]) -> Date? {
        let text = (event["timestamp"] ?? event["created_at"] ?? event["occurred_at"]) as? String
        guard let text else { return nil }
        return parseTimestamp(text)
    }

    public static func parseTimestamp(_ text: String) -> Date? {
        for options: ISO8601DateFormatter.Options in [
            [.withInternetDateTime, .withFractionalSeconds],
            [.withInternetDateTime]
        ] {
            let formatter = ISO8601DateFormatter()
            formatter.formatOptions = options
            formatter.timeZone = TimeZone(secondsFromGMT: 0)
            if let date = formatter.date(from: text) { return date }
        }
        return nil
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

public struct AgentTaskDateGroup: Identifiable, Equatable, Sendable {
    public let id: String
    public let date: Date?
    public var tasks: [NativeAgentTask]

    public init(id: String, date: Date?, tasks: [NativeAgentTask]) {
        self.id = id
        self.date = date
        self.tasks = tasks
    }
}

public struct AgentTaskHistoryItem: Identifiable, Equatable, Sendable {
    public let id: Int
    public let eventType: String
    public let timestamp: Date?
    public let icon: AgentTaskHistoryIcon
    public let title: String
    public let detail: String
    public let rawStatus: String?

    public init(id: Int, eventType: String, timestamp: Date?, icon: AgentTaskHistoryIcon, title: String, detail: String, rawStatus: String? = nil) {
        self.id = id
        self.eventType = eventType
        self.timestamp = timestamp
        self.icon = icon
        self.title = title
        self.detail = detail
        self.rawStatus = rawStatus
    }
}

public enum AgentTaskHistoryIcon: Equatable, Sendable {
    case started
    case observation
    case awaitingApproval
    case pending
    case running
    case awaitingInput
    case completed
    case failed
    case unknown

    public init(status: String) {
        let normalized = status.lowercased().filter(\.isLetter)
        switch normalized {
        case "pending": self = .pending
        case "running": self = .running
        case "awaitingapproval", "approvalrequired": self = .awaitingApproval
        case "awaitinginput", "awaitinguserinput": self = .awaitingInput
        case "completed", "complete", "finished", "success", "succeeded": self = .completed
        case "failed", "failure", "error": self = .failed
        default: self = .unknown
        }
    }

    public var systemImage: String {
        switch self {
        case .started: "play.circle.fill"
        case .observation: "eye.fill"
        case .awaitingApproval: "hand.raised.fill"
        case .pending: "clock.fill"
        case .running: "arrow.triangle.2.circlepath"
        case .awaitingInput: "text.cursor"
        case .completed: "checkmark.circle.fill"
        case .failed: "xmark.circle.fill"
        case .unknown: "questionmark.circle"
        }
    }

    public var accessibilityLabel: String {
        switch self {
        case .started: "開始"
        case .observation: "観測結果"
        case .awaitingApproval: "承認待ち"
        case .pending: "待機中"
        case .running: "実行中"
        case .awaitingInput: "入力待ち"
        case .completed: "完了"
        case .failed: "失敗"
        case .unknown: "状態不明"
        }
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
