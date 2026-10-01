import Foundation
import Testing
@testable import MikomaiDesktopCore

struct ChatSubmissionQueueTests {
    @Test func acceptsInputDuringGenerationAndCancellationWithoutOverlappingRequests() throws {
        let session = UUID()
        var lifecycle = ChatResponseLifecycle()
        let firstCandidate = lifecycle.begin(sessionID: session)
        let first = try #require(firstCandidate)
        var queue = ChatSubmissionQueue()
        let attachments = [PendingAttachment(name: "notes.md", text: "VLAN 10")]
        let waiting = QueuedChatSubmission(sessionID: session, prompt: "次の質問", attachments: attachments)
        queue.enqueue(waiting)
        #expect(queue.takeNext(isWorking: lifecycle.isWorking, validSessionIDs: [session]) == nil)
        lifecycle.cancel()
        queue.enqueue(QueuedChatSubmission(sessionID: session, prompt: "停止中の質問", attachments: []))
        #expect(queue.takeNext(isWorking: lifecycle.isWorking, validSessionIDs: [session]) == nil)
        #expect(queue.submissions.count == 2)
        lifecycle.finish(first)
        let nextCandidate = queue.takeNext(isWorking: lifecycle.isWorking, validSessionIDs: [session])
        let next = try #require(nextCandidate)
        #expect(next == waiting)
        #expect(next.attachments.first?.text == "VLAN 10")
        let secondCandidate = lifecycle.begin(sessionID: next.sessionID)
        let second = try #require(secondCandidate)
        lifecycle.finish(first) // A stale completion must not release the new request.
        #expect(queue.takeNext(isWorking: lifecycle.isWorking, validSessionIDs: [session]) == nil)
        lifecycle.finish(second)
        #expect(queue.takeNext(isWorking: lifecycle.isWorking, validSessionIDs: [session])?.prompt == "停止中の質問")
    }

    @Test func preservesFIFOAndOriginalConversationsAndAllowsRemovingPendingInput() {
        let a = UUID(), b = UUID()
        var queue = ChatSubmissionQueue()
        let first = QueuedChatSubmission(sessionID: a, prompt: "A", attachments: [])
        let removed = QueuedChatSubmission(sessionID: b, prompt: "取り消す", attachments: [])
        let last = QueuedChatSubmission(sessionID: b, prompt: "", attachments: [PendingAttachment(name: "config.txt", text: "config")])
        queue.enqueue(first); queue.enqueue(removed); queue.enqueue(last)
        queue.remove(id: removed.id)
        #expect(queue.takeNext(isWorking: false, validSessionIDs: [a, b]) == first)
        #expect(queue.takeNext(isWorking: false, validSessionIDs: [a, b]) == last)
        #expect(queue.takeNext(isWorking: false, validSessionIDs: [a, b]) == nil)
    }

    @Test func dropsDeletedConversationsAndRejectsEmptySubmissions() {
        let deleted = UUID(), valid = UUID()
        var queue = ChatSubmissionQueue()
        queue.enqueue(QueuedChatSubmission(sessionID: valid, prompt: " \n", attachments: []))
        #expect(queue.submissions.isEmpty)
        queue.enqueue(QueuedChatSubmission(sessionID: deleted, prompt: "削除された会話", attachments: []))
        queue.enqueue(QueuedChatSubmission(sessionID: valid, prompt: "残す", attachments: []))
        #expect(queue.takeNext(isWorking: false, validSessionIDs: [valid])?.prompt == "残す")
        queue.enqueue(QueuedChatSubmission(sessionID: valid, prompt: "削除", attachments: []))
        queue.remove(sessionID: valid)
        #expect(queue.submissions.isEmpty)
    }
}

struct ExecutionTerminalPresentationTests {
    @Test func recognizesNativeAndLegacyProbeNamesAndKeepsFailureOutput() {
        for tool in ["self_network_ping", "self_network_traceroute", "self_ping", "self_trace"] {
            #expect(AgentToolResult.isLocalProbe(tool: tool))
        }
        #expect(!AgentToolResult.isLocalProbe(tool: "get_state"))
        let output = AgentToolResult.terminalOutput(stdout: "10 packets transmitted, 0 packets received\n", stderr: "ping: timeout\n")
        #expect(output == "10 packets transmitted, 0 packets received\nping: timeout\n")
        #expect(AgentToolResult.terminalOutput(stdout: "", stderr: "traceroute: host unknown") == "traceroute: host unknown")
        #expect(AgentToolResult.terminalOutput(stdout: "reply", stderr: "warning") == "reply\nwarning")
    }
}
