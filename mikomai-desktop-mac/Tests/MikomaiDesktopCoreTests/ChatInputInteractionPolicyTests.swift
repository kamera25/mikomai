import Testing
@testable import MikomaiDesktopCore

@Suite
struct ChatInputInteractionPolicyTests {
    @Test func emptySuggestionsCloseTheListWithoutResurrectionOnHostUpdates() {
        var visibility = ChatSuggestionVisibilityState()
        visibility.updateForInput(hasMentionQuery: true, candidateCount: 1)
        #expect(visibility.isVisible)

        visibility.updateCandidates(count: 2)
        #expect(visibility.isVisible)
        visibility.updateCandidates(count: 0)
        #expect(!visibility.isVisible)

        visibility.updateCandidates(count: 1)
        #expect(!visibility.isVisible)
        visibility.updateForInput(hasMentionQuery: true, candidateCount: 1)
        #expect(visibility.isVisible)
    }

    @Test func escapeDismissesSuggestionsWithoutChangingDraftOrSubmitting() {
        var visibility = ChatSuggestionVisibilityState()
        let originalDraft = "@router"
        visibility.updateForInput(hasMentionQuery: true, candidateCount: 1)
        visibility.dismissForEscape()

        #expect(!visibility.isVisible)
        #expect(originalDraft == "@router")
        #expect(ChatSubmissionPolicy.shouldSubmit(prompt: originalDraft, attachmentCount: 0, isWorking: false))
    }

    @Test func sendAndStopControlsMatchChatInputState() {
        #expect(ChatSubmissionPolicy.normalizedPrompt("  hello \n") == "hello")
        #expect(ChatSubmissionPolicy.hasContent(prompt: " \n", attachmentCount: 1))
        #expect(!ChatSubmissionPolicy.hasContent(prompt: " \n", attachmentCount: 0))
        #expect(ChatSubmissionPolicy.shouldSubmit(prompt: "hello", attachmentCount: 0, isWorking: false))
        #expect(!ChatSubmissionPolicy.shouldSubmit(prompt: "hello", attachmentCount: 0, isWorking: true))
        #expect(ChatSubmissionPolicy.canStop(isWorking: true, isCancelling: false))
        #expect(!ChatSubmissionPolicy.canStop(isWorking: false, isCancelling: false))
        #expect(!ChatSubmissionPolicy.canStop(isWorking: true, isCancelling: true))
    }
}
