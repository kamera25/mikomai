import Foundation

public struct QueuedChatSubmission: Identifiable, Equatable {
    public let id: UUID
    public let sessionID: UUID
    public let prompt: String
    public let attachments: [PendingAttachment]

    public init(id: UUID = UUID(), sessionID: UUID, prompt: String, attachments: [PendingAttachment]) {
        self.id = id
        self.sessionID = sessionID
        self.prompt = prompt
        self.attachments = attachments
    }
}

/// Pending input is kept outside conversation history until the engine is free.
public struct ChatSubmissionQueue: Equatable {
    public private(set) var submissions: [QueuedChatSubmission] = []
    public init() {}

    public mutating func enqueue(_ submission: QueuedChatSubmission) {
        guard ChatSubmissionPolicy.hasContent(prompt: submission.prompt, attachmentCount: submission.attachments.count) else { return }
        submissions.append(submission)
    }

    public mutating func takeNext(isWorking: Bool, validSessionIDs: Set<UUID>) -> QueuedChatSubmission? {
        guard !isWorking else { return nil }
        submissions.removeAll { !validSessionIDs.contains($0.sessionID) }
        return submissions.isEmpty ? nil : submissions.removeFirst()
    }

    public mutating func remove(id: UUID) { submissions.removeAll { $0.id == id } }
    public mutating func remove(sessionID: UUID) { submissions.removeAll { $0.sessionID == sessionID } }
}
