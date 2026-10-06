import Foundation
import Testing
@testable import MikomaiDesktopCore

@Suite
struct NativeFeatureLogicTests {
    @Test func chatSessionSelectionAndDeletionKeepAnActiveSession() {
        let first = ChatSession(title: "First")
        let second = ChatSession(title: "Second")
        var state = ChatSessionState(sessions: [first, second], activeSessionID: UUID())

        #expect(state.activeSessionID == first.id)
        state.select(second.id)
        #expect(state.activeSessionID == second.id)
        state.select(UUID())
        #expect(state.activeSessionID == second.id)

        state.delete(second.id)
        #expect(state.activeSessionID == first.id)
        #expect(state.sessions.contains { $0.id == state.activeSessionID })
    }

    @Test func deletingLastSessionCreatesASelectableReplacement() {
        let onlySession = ChatSession(title: "Only")
        var state = ChatSessionState(sessions: [onlySession], activeSessionID: onlySession.id)

        state.delete(onlySession.id)

        #expect(state.sessions.count == 1)
        #expect(state.sessions[0].id != onlySession.id)
        #expect(state.activeSessionID == state.sessions[0].id)
    }

    @Test func sessionRenameTrimsWhitespaceAndIgnoresEmptyTitle() {
        let session = ChatSession(title: "Original")
        var state = ChatSessionState(sessions: [session], activeSessionID: session.id)

        state.rename(session.id, to: "  VLAN notes \n")
        #expect(state.sessions[0].title == "VLAN notes")
        state.rename(session.id, to: " \n ")
        #expect(state.sessions[0].title == "VLAN notes")
    }

    @Test func chatCanSubmitBeforeModelLoadsButNotWhileWorkingOrWithoutContent() {
        #expect(ChatSubmissionPolicy.shouldSubmit(prompt: "hello", attachmentCount: 0, isWorking: false))
        #expect(ChatSubmissionPolicy.shouldSubmit(prompt: "  ", attachmentCount: 1, isWorking: false))
        #expect(!ChatSubmissionPolicy.shouldSubmit(prompt: "  ", attachmentCount: 0, isWorking: false))
        #expect(ChatSubmissionPolicy.shouldSubmit(prompt: "hello", attachmentCount: 0, isWorking: true))
    }

    @Test func attachmentAcceptsTheExactPerFileAndAggregateLimits() throws {
        let data = Data(repeating: 0x61, count: TextAttachmentPolicy.maxFileBytes)
        let attachment = try TextAttachmentPolicy.prepare(name: "manual.MD", data: data)
        #expect(attachment.byteCount == 64 * 1024)

        let totalBoundary = try TextAttachmentPolicy.prepare(
            name: "second.txt",
            data: Data(repeating: 0x62, count: 64 * 1024),
            existingNames: ["manual.MD"],
            currentTotalBytes: 64 * 1024
        )
        #expect(totalBoundary.byteCount == 64 * 1024)
    }

    @Test func attachmentRejectsFilesOverEachSizeLimit() {
        expectAttachmentError(.tooLarge) {
            try TextAttachmentPolicy.prepare(
            name: "large.txt",
            data: Data(repeating: 0x61, count: TextAttachmentPolicy.maxFileBytes + 1)
            )
        }

        expectAttachmentError(.totalTooLarge) {
            try TextAttachmentPolicy.prepare(
            name: "second.txt",
            data: Data(repeating: 0x61, count: 1),
            currentTotalBytes: TextAttachmentPolicy.maxTotalBytes
            )
        }
    }

    @Test func attachmentRejectsDuplicateNamesUnsupportedTypesInvalidUTF8AndNul() {
        expectAttachmentError(.duplicate) {
            try TextAttachmentPolicy.prepare(
            name: "manual.txt", data: Data("text".utf8), existingNames: ["manual.txt"]
            )
        }
        expectAttachmentError(.unsupportedType) {
            try TextAttachmentPolicy.prepare(name: "image.png", data: Data())
        }
        expectAttachmentError(.invalidEncoding) {
            try TextAttachmentPolicy.prepare(name: "bad.txt", data: Data([0xC3, 0x28]))
        }
        expectAttachmentError(.containsNull) {
            try TextAttachmentPolicy.prepare(name: "bad.txt", data: Data([0x61, 0x00]))
        }
    }

    private func expectAttachmentError(
        _ expected: AttachmentReadError,
        operation: () throws -> PendingAttachment
    ) {
        do {
            _ = try operation()
            Issue.record("Expected attachment error: \(expected)")
        } catch let error as AttachmentReadError {
            #expect(error == expected)
        } catch {
            Issue.record("Unexpected error: \(error)")
        }
    }
}
