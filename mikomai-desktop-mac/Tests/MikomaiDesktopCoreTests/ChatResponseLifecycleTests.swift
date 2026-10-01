import Foundation
import Testing
@testable import MikomaiDesktopCore

struct ChatResponseLifecycleTests {
    @Test func progressBelongsOnlyToTheSubmittingConversation() throws {
        var state = ChatResponseLifecycle()
        let session = UUID()
        #expect(!state.showsProgress(in: session))
        let requestCandidate = state.begin(sessionID: session)
        let request = try #require(requestCandidate)
        #expect(state.showsProgress(in: session))
        #expect(!state.showsProgress(in: UUID()))
        #expect(!state.showsProgress(in: nil))
        state.finish(request)
        #expect(!state.showsProgress(in: session))
        #expect(!state.isWorking)
        let submission = state.begin(sessionID: UUID())
        #expect(submission != nil)
    }

    @Test func cancellationRejectsChunksAndAllowsResubmissionAfterCleanup() throws {
        var state = ChatResponseLifecycle()
        let session = UUID()
        let oldCandidate = state.begin(sessionID: session)
        let old = try #require(oldCandidate)
        state.cancel()
        #expect(state.isCancelling)
        #expect(!state.acceptsChunk(for: old))
        // The shared inference engine must finish before another request starts.
        let submission = state.begin(sessionID: session)
        #expect(submission == nil)
        state.finish(old)
        #expect(!state.isCancelling)
        let nextCandidate = state.begin(sessionID: session)
        let next = try #require(nextCandidate)
        #expect(state.acceptsChunk(for: next))
        #expect(!state.acceptsChunk(for: old))
        state.finish(old)
        #expect(state.isWorking)
        state.finish(next)
        #expect(!state.acceptsChunk(for: next))
        #expect(!state.isWorking)
    }

    @Test func finalAnswerReplacesProgressAndPartialChunks() {
        var state = ChatResponseLifecycle()
        _ = state.begin(sessionID: UUID())
        #expect(state.finalText(streamed: "ネットワーク機器の状態を確認しています…\n途中", answer: "完成した回答") == "完成した回答")
        #expect(state.finalText(streamed: "途中", answer: "エラー: モデル未ロード") == "エラー: モデル未ロード")
        state.cancel()
        #expect(state.finalText(streamed: "", answer: "遅れて届いた回答") == "生成を停止しました。")
        #expect(state.finalText(streamed: "途中", answer: "遅れて届いた回答") == "途中\n\n(生成を停止しました)")
    }

    @Test func deletedConversationAndFailedRequestStillReleaseTheComposer() throws {
        var state = ChatResponseLifecycle()
        let requestCandidate = state.begin(sessionID: UUID())
        let request = try #require(requestCandidate)
        #expect(!state.showsProgress(in: nil))
        state.finish(request)
        let submission = state.begin(sessionID: UUID())
        #expect(submission != nil)
    }
}
