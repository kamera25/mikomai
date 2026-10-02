import Testing
@testable import MikomaiDesktopCore

struct ChatHistoryPolicyTests {
    private let messages: [ChatMessage] = [
        ChatMessage(role: .user, text: "古い質問"),
        ChatMessage(role: .assistant, text: "古い回答"),
        ChatMessage(role: .user, text: "ping 8.8.8.8"),
        ChatMessage(role: .assistant, text: "パケットロスは0.0%です。")
    ]

    @Test func zeroAndNegativeLimitsDisableHistory() {
        for limit in [0, -1, Int.min] {
            #expect(ChatHistoryPolicy.context(messages: messages, turnLimit: limit).isEmpty)
        }
    }

    @Test func keepsOnlyRequestedRecentTurnsInOrder() {
        #expect(ChatHistoryPolicy.context(messages: messages, turnLimit: 1)
            == "ユーザー: ping 8.8.8.8\nMIKOMAI: パケットロスは0.0%です。")
        let all = "ユーザー: 古い質問\nMIKOMAI: 古い回答\nユーザー: ping 8.8.8.8\nMIKOMAI: パケットロスは0.0%です。"
        for limit in [2, 20, Int.max] {
            #expect(ChatHistoryPolicy.context(messages: messages, turnLimit: limit) == all)
        }
    }

    @Test func countsUserTurnsInsteadOfIndividualMessages() {
        let irregular = messages + [
            ChatMessage(role: .assistant, text: "追加回答"),
            ChatMessage(role: .user, text: "未回答の質問"),
            ChatMessage(role: .assistant, text: " \n")
        ]
        #expect(ChatHistoryPolicy.context(messages: irregular, turnLimit: 1)
            == "ユーザー: 未回答の質問")
        #expect(ChatHistoryPolicy.context(messages: irregular, turnLimit: 2)
            == "ユーザー: ping 8.8.8.8\nMIKOMAI: パケットロスは0.0%です。\nMIKOMAI: 追加回答\nユーザー: 未回答の質問")
    }

    @Test func emptyHistoryAndOrphanRepliesDoNotCreateTurns() {
        #expect(ChatHistoryPolicy.context(messages: [], turnLimit: 5).isEmpty)
        let orphan = ChatMessage(role: .assistant, text: "質問のない回答")
        #expect(ChatHistoryPolicy.context(messages: [orphan], turnLimit: 5).isEmpty)
        #expect(ChatHistoryPolicy.context(messages: [orphan] + messages, turnLimit: 5)
            == ChatHistoryPolicy.context(messages: messages, turnLimit: 5))
    }
}
