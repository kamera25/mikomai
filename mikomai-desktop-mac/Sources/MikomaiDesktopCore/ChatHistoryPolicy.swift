import Foundation

public enum ChatHistoryPolicy {
    /// A turn starts with a user message and includes the following assistant replies.
    /// Build this context before appending the current request to the session.
    public static func context(messages: [ChatMessage], turnLimit: Int) -> String {
        guard turnLimit > 0 else { return "" }
        let turnStarts = messages.indices.filter { messages[$0].role == .user }
        guard let start = turnStarts.suffix(turnLimit).first else { return "" }
        return messages[start...].compactMap { message in
            guard !message.text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
                return nil
            }
            return "\(message.role == .user ? "ユーザー" : "MIKOMAI"): \(message.text)"
        }.joined(separator: "\n")
    }
}
