import Testing
import AppKit
@testable import MikomaiDesktopCore

@Suite
struct ChatPresentationPolicyTests {
    @Test func messageGrowthAtBottomKeepsFollowingEnabled() {
        var state = ChatScrollFollowState()
        state.observe(contentTop: 0, isAtBottom: true)
        state.observe(contentTop: -24, isAtBottom: true)
        #expect(state.followsOutput)
    }

    @Test func upwardScrollStopsFollowingEvenWhenTopPreferenceArrivesFirst() {
        var state = ChatScrollFollowState()
        state.observe(contentTop: 0, isAtBottom: true)
        state.observe(contentTop: 18, isAtBottom: true)
        #expect(state.followsOutput)

        state.updateViewport(isAtBottom: false)
        #expect(!state.followsOutput)
    }

    @Test func resumeAndSessionChangeRestoreFollowingAndClearOffsets() {
        var state = ChatScrollFollowState()
        state.observe(contentTop: 0, isAtBottom: true)
        state.observe(contentTop: 3, isAtBottom: false)
        #expect(!state.followsOutput)

        state.resume()
        #expect(state.followsOutput)
        state.resetForSessionChange()
        #expect(state.followsOutput)
        #expect(state.lastContentTop == nil)
    }

    @Test func paneWidthsClampAndCloseAtTheSharedMinimum() {
        #expect(PaneResizePolicy.maximumWidth(containerWidth: 1200, reservedWidth: 700, lowerBound: 180, upperBound: 420) == 420)
        #expect(PaneResizePolicy.maximumWidth(containerWidth: 700, reservedWidth: 600, lowerBound: 180, upperBound: 420) == 180)
        #expect(PaneResizePolicy.clampedWidth(140, maximumWidth: 500) == 180)
        #expect(PaneResizePolicy.clampedWidth(640, maximumWidth: 500) == 500)
        #expect(PaneResizePolicy.shouldClose(startWidth: 200, translation: -21, isHistoryPane: true))
        #expect(!PaneResizePolicy.shouldClose(startWidth: 200, translation: -20, isHistoryPane: true))
        #expect(PaneResizePolicy.shouldClose(startWidth: 200, translation: 21, isHistoryPane: false))
    }

    @Test func markdownParserProducesExpectedChatBlocks() {
        let blocks = ChatMarkdownParser.parse("""
        # 見出し
        説明の一行目
        説明の二行目

        - 項目
        > 引用
        ---
        ```ios
        interface vlan 20
        ```
        """)

        #expect(blocks.map(\.kind) == [
            .heading(1, "見出し"),
            .paragraph("説明の一行目 説明の二行目"),
            .bullet("項目"),
            .quote("引用"),
            .separator,
            .code("ios", "interface vlan 20")
        ])
    }

    @MainActor @Test func clipboardCopiesJapaneseMarkdownAndReplacesExistingContents() {
        let pasteboard = TestClipboardBoard(contents: "古い内容")

        let message = "## VLAN 設定\n\n```ios\ninterface vlan 20\n description 管理用\n```"
        #expect(ChatMessageClipboard.copy(text: message, to: pasteboard))
        #expect(pasteboard.contents == message)
        #expect(pasteboard.clearCount == 1)
    }
}

@MainActor
private final class TestClipboardBoard: ChatClipboardBoard {
    var contents: String?
    private(set) var clearCount = 0

    init(contents: String? = nil) { self.contents = contents }

    func clearForCopy() {
        clearCount += 1
        contents = nil
    }

    func setString(_ string: String, forType type: NSPasteboard.PasteboardType) -> Bool {
        guard type == .string else { return false }
        contents = string
        return true
    }
}
