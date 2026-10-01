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
