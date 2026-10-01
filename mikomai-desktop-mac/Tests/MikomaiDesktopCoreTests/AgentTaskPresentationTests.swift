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
