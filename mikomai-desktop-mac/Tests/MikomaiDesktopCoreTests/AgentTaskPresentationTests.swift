import Foundation
import Testing
@testable import MikomaiDesktopCore

@Suite
struct AgentTaskPresentationTests {
    @Test func formatsGoalCurrentStatusAndObservedToolOutput() throws {
        let response = #"{"events":[{"event_type":"task_started","goal":"core-router CPUを確認"},{"event_type":"observation","evidence":{"raw":"CPU usage: 91%","source":{"tool_name":"get_state"}}},{"event_type":"state_updated","status":"Running"}]}"#

        let lines = AgentTaskHistoryPresentation.lines(from: response, fallbackGoal: "fallback goal")

        #expect(lines == [
            "開始: core-router CPUを確認",
            "観測結果:\nCPU usage: 91%",
            "状態: Running"
        ])
    }

    @Test func fallsBackToTaskGoalAndKeepsFinishedAnswerReadable() {
        let response = #"{"events":[{"event_type":"task_started"},{"event_type":"finished","answer":"CPU使用率は91%です。"}]}"#

        let lines = AgentTaskHistoryPresentation.lines(from: response, fallbackGoal: "router statusを調べる")

        #expect(lines.first == "開始: router statusを調べる")
        #expect(lines.last == "完了:\nCPU使用率は91%です。")
    }

    @Test func parsesFractionalRFC3339TimesAndUsesOneIconMappingForTaskStates() throws {
        let response = #"{"events":[{"event_type":"state_updated","status":"pending","timestamp":"2026-10-01T15:04:05.123Z"},{"event_type":"state_updated","status":"awaiting_approval"},{"event_type":"state_updated","status":"complete"},{"event_type":"state_updated","status":"completed"},{"event_type":"state_updated","status":"unknown_value"},{"event_type":"finished","error":"agent failed"}]}"#

        let items = AgentTaskHistoryPresentation.items(from: response, fallbackGoal: "fallback")

        #expect(items.count == 6)
        #expect(items[0].timestamp != nil)
        #expect(items[0].icon == .pending)
        #expect(items[1].icon == .awaitingApproval)
        #expect(items[2].icon == .completed)
        #expect(items[2].icon.systemImage == items[3].icon.systemImage)
        #expect(items[2].icon.accessibilityLabel == "完了")
        #expect(items[4].icon == .unknown)
        #expect(items[5].icon == .failed)
        #expect(items[5].title == "失敗")
        #expect(items[5].detail == "agent failed")
    }

    @Test func parsesTauriLegacyEventTypesWithProperLabelsAndIcons() throws {
        let tauriResponse = """
        {
          "events": [
            {"event_type": "task_started", "timestamp": "2026-09-05T06:34:41.176Z"},
            {"event_type": "goal_set", "goal": "F220のVLAN設定方法を教えて", "timestamp": "2026-09-05T06:34:41.176Z"},
            {"event_type": "decision", "action_type": "OBSERVE", "objective": "F220のVLAN設定を検索", "reason": ["NW-DBを検索する必要がある"], "timestamp": "2026-09-05T06:34:58.971Z"},
            {"event_type": "action", "tool": "query_nw_db", "target": "DB", "parameters": {"query": "vlan"}, "timestamp": "2026-09-05T06:34:59.000Z"},
            {"event_type": "result", "tool": "query_nw_db", "success": true, "observation": {"raw": "vlan database", "source": {"tool_name": "query_nw_db"}}, "timestamp": "2026-09-05T06:35:09.909Z"},
            {"event_type": "result", "tool": "netmiko", "success": false, "error": "ConnectionFailed", "timestamp": "2026-09-05T06:35:15.000Z"},
            {"event_type": "finished", "reason": "調査完了", "timestamp": "2026-09-05T06:35:20.000Z"}
          ]
        }
        """

        let items = AgentTaskHistoryPresentation.items(from: tauriResponse, fallbackGoal: "fallback")
        #expect(items.count == 7)

        #expect(items[0].eventType == "task_started")
        #expect(items[0].title == "開始")
        #expect(items[0].detail == "fallback")

        #expect(items[1].eventType == "goal_set")
        #expect(items[1].title == "目標設定")
        #expect(items[1].detail == "F220のVLAN設定方法を教えて")

        #expect(items[2].eventType == "decision")
        #expect(items[2].title == "判断: OBSERVE")
        #expect(items[2].detail.contains("F220のVLAN設定を検索"))
        #expect(items[2].detail.contains("NW-DBを検索する必要がある"))

        #expect(items[3].eventType == "action")
        #expect(items[3].title == "実行: query_nw_db")
        #expect(items[3].detail.contains("DB"))
        #expect(items[3].detail.contains("query_nw_db"))

        #expect(items[4].eventType == "result")
        #expect(items[4].title == "成功: query_nw_db")
        #expect(items[4].icon == .completed)
        #expect(items[4].detail == "vlan database")

        #expect(items[5].eventType == "result")
        #expect(items[5].title.contains("失敗"))
        #expect(items[5].icon == .failed)
        #expect(items[5].detail == "ConnectionFailed")

        #expect(items[6].eventType == "finished")
        #expect(items[6].title == "完了")
        #expect(items[6].icon == .completed)
        #expect(items[6].detail == "調査完了")
    }

    @Test func formatsStructuredStateUpdatedJSONWithPhaseAndNextAction() throws {
        let json = """
        {
          "events": [
            {
              "event_type": "state_updated",
              "status": "{\\"detail\\":\\"目的と収集済みの情報を確認しています\\",\\"nextAction\\":\\"観測結果から次の操作を選択\\",\\"phase\\":\\"計画\\"}",
              "timestamp": "2026-10-02T07:30:52.691Z"
            }
          ]
        }
        """

        let items = AgentTaskHistoryPresentation.items(from: json, fallbackGoal: "fallback")
        #expect(items.count == 1)
        #expect(items[0].title == "状態 (計画)")
        #expect(items[0].detail.contains("目的と収集済みの情報を確認しています"))
        #expect(items[0].detail.contains("次: 観測結果から次の操作を選択"))
    }

    @Test func preservesUnknownEventTypeInsteadOfDropping() throws {
        let json = """
        {
          "events": [
            {
              "event_type": "custom_extension_event",
              "detail": "カスタムログメッセージ",
              "timestamp": "2026-10-02T07:30:52Z"
            }
          ]
        }
        """

        let items = AgentTaskHistoryPresentation.items(from: json, fallbackGoal: "fallback")
        #expect(items.count == 1)
        #expect(items[0].title == "custom_extension_event")
        #expect(items[0].detail == "カスタムログメッセージ")
        #expect(items[0].icon == .unknown)
    }

    @Test func dateLabelsAndDayBoundariesUseTheCalendarTimezone() throws {
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = try #require(TimeZone(identifier: "Asia/Tokyo"))
        let locale = Locale(identifier: "ja_JP")
        let now = try #require(AgentTaskHistoryPresentation.parseTimestamp("2026-10-01T15:00:00Z"))
        let thisYear = try #require(AgentTaskHistoryPresentation.parseTimestamp("2026-10-01T15:04:05.123Z"))
        let lastYear = try #require(AgentTaskHistoryPresentation.parseTimestamp("2025-10-01T15:04:05Z"))

        #expect(AgentTaskHistoryPresentation.timeLabel(thisYear, calendar: calendar, locale: locale) == "00:04")
        #expect(AgentTaskHistoryPresentation.dateLabel(thisYear, now: now, calendar: calendar, locale: locale) == "10月2日")
        #expect(AgentTaskHistoryPresentation.dateLabel(lastYear, now: now, calendar: calendar, locale: locale) == "2025年10月2日")

        let yearEndNow = try #require(AgentTaskHistoryPresentation.parseTimestamp("2026-12-31T15:00:00Z"))
        #expect(AgentTaskHistoryPresentation.dateLabel(thisYear, now: yearEndNow, calendar: calendar, locale: locale) == "2026年10月2日")

        let midnightBefore = try #require(AgentTaskHistoryPresentation.parseTimestamp("2026-10-01T14:59:00Z"))
        let midnightAfter = try #require(AgentTaskHistoryPresentation.parseTimestamp("2026-10-01T15:00:00Z"))
        let items = [
            AgentTaskHistoryItem(id: 0, eventType: "state_updated", timestamp: midnightBefore, icon: .pending, title: "状態", detail: "pending"),
            AgentTaskHistoryItem(id: 1, eventType: "state_updated", timestamp: nil, icon: .unknown, title: "状態", detail: "unknown"),
            AgentTaskHistoryItem(id: 2, eventType: "state_updated", timestamp: midnightAfter, icon: .running, title: "状態", detail: "running"),
            AgentTaskHistoryItem(id: 3, eventType: "state_updated", timestamp: midnightBefore, icon: .pending, title: "状態", detail: "pending")
        ]
        #expect(AgentTaskHistoryPresentation.startsNewDay(at: 0, in: items, calendar: calendar))
        #expect(!AgentTaskHistoryPresentation.startsNewDay(at: 1, in: items, calendar: calendar))
        #expect(AgentTaskHistoryPresentation.startsNewDay(at: 2, in: items, calendar: calendar))
        #expect(AgentTaskHistoryPresentation.startsNewDay(at: 3, in: items, calendar: calendar))
    }

    @Test func preservesInvalidTimestampAndRawJSONFallbacks() throws {
        let invalidTimestamp = #"{"events":[{"event_type":"task_started","timestamp":"not-a-date"}]}"#
        let item = try #require(AgentTaskHistoryPresentation.items(from: invalidTimestamp, fallbackGoal: "fallback").first)
        #expect(item.timestamp == nil)
        #expect(item.detail == "fallback")
        #expect(AgentTaskHistoryPresentation.items(from: "", fallbackGoal: "fallback").isEmpty)
        #expect(AgentTaskHistoryPresentation.lines(from: "legacy raw text", fallbackGoal: "fallback") == ["legacy raw text"])
    }

    @Test func groupsAgentSummariesByLocalUpdateDayWithoutReordering() throws {
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = try #require(TimeZone(identifier: "Asia/Tokyo"))
        let tasks = [
            NativeAgentTask(taskId: "newer", goal: "newer", status: "running", startedAt: "", lastEventAt: "2026-10-02T02:30:00Z", eventCount: 2),
            NativeAgentTask(taskId: "same-day", goal: "same day", status: "pending", startedAt: "", lastEventAt: "2026-10-01T15:04:05.123Z", eventCount: 1),
            NativeAgentTask(taskId: "previous-day", goal: "previous day", status: "completed", startedAt: "", lastEventAt: "2026-10-01T14:59:00Z", eventCount: 4),
            NativeAgentTask(taskId: "bad-date", goal: "bad date", status: "unknown", startedAt: "", lastEventAt: "not-a-date", eventCount: 0)
        ]

        let groups = AgentTaskHistoryPresentation.taskDateGroups(tasks, calendar: calendar)

        #expect(groups.count == 3)
        #expect(groups[0].tasks.map(\.id) == ["newer", "same-day"])
        #expect(groups[1].tasks.map(\.id) == ["previous-day"])
        #expect(groups[2].date == nil)
        #expect(groups[2].tasks.map(\.id) == ["bad-date"])
        let time = try #require(AgentTaskHistoryPresentation.parseTimestamp(tasks[0].lastEventAt))
        #expect(AgentTaskHistoryPresentation.timeLabel(time, calendar: calendar, locale: Locale(identifier: "ja_JP")) == "11:30")
        #expect(AgentTaskHistoryPresentation.taskUpdateTimeLabel(tasks[0].lastEventAt, calendar: calendar, locale: Locale(identifier: "ja_JP")) == "11:30")
        #expect(AgentTaskHistoryPresentation.taskUpdateTimeLabel("not-a-date", calendar: calendar, locale: Locale(identifier: "ja_JP")) == "時刻不明")
        #expect(AgentTaskHistoryPresentation.dateLabel(try #require(groups[0].date), now: time, calendar: calendar, locale: Locale(identifier: "ja_JP")) == "10月2日")
    }
}

@Suite struct AgentProgressTests {
    @Test func progressTransportIsSeparateFromAnswerAndPersistsAcrossReload() throws {
        let chunk = AgentProgressEntry.streamPrefix + #"{"phase":"実行","nextAction":"結果を確認","detail":"get_state · sw1"}"#
        let entry = try #require(AgentProgressEntry.parse(chunk))
        var message = ChatMessage(role: .assistant, text: "診断完了")
        message.agentGoal = "sw1を診断"
        message.agentProgress = [entry]
        let restored = try JSONDecoder().decode(ChatMessage.self, from: JSONEncoder().encode(message))
        #expect(restored == message)
        #expect(restored.text == "診断完了")
        #expect(restored.agentProgress?.first?.detail == "get_state · sw1")
        #expect(AgentProgressEntry.parse("通常の回答") == nil)
        #expect(AgentProgressEntry.parse(AgentProgressEntry.streamPrefix + "broken") == nil)
    }

    @Test func legacyMessagesRemainReadableWithoutProgress() throws {
        let message = try JSONDecoder().decode(ChatMessage.self, from: Data(#"{"role":"assistant","text":"以前の回答"}"#.utf8))
        #expect(message.agentProgress == nil)
        #expect(message.text == "以前の回答")
    }
}
