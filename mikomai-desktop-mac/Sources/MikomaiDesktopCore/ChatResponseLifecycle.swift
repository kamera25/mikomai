import Foundation

/// Owns one FFI request until it returns, including cancellation cleanup.
public struct ChatResponseLifecycle: Equatable, Sendable {
    public private(set) var requestID: UUID?
    public private(set) var sessionID: UUID?
    public private(set) var isCancelling = false
    public var isWorking: Bool { requestID != nil }
    public init() {}

    @discardableResult
    public mutating func begin(sessionID: UUID) -> UUID? {
        guard !isWorking else { return nil }
        let id = UUID()
        requestID = id
        self.sessionID = sessionID
        isCancelling = false
        return id
    }

    public func showsProgress(in sessionID: UUID?) -> Bool {
        isWorking && self.sessionID == sessionID
    }

    public func acceptsChunk(for requestID: UUID) -> Bool {
        self.requestID == requestID && !isCancelling
    }

    public func finalText(streamed: String, answer: String) -> String {
        guard isCancelling else { return answer }
        let partial = streamed.trimmingCharacters(in: .whitespacesAndNewlines)
        return partial.isEmpty ? "生成を停止しました。" : partial + "\n\n(生成を停止しました)"
    }

    public mutating func cancel() {
        guard isWorking else { return }
        isCancelling = true
    }

    public mutating func finish(_ requestID: UUID) {
        guard self.requestID == requestID else { return }
        self = Self()
    }
}
