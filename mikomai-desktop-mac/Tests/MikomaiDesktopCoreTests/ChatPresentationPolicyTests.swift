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

    @Test func responsiveTilingDetectsCompactWidthAndSideSnapping() {
        #expect(PaneResizePolicy.isCompactWidth(960))
        #expect(PaneResizePolicy.isCompactWidth(720))
        #expect(!PaneResizePolicy.isCompactWidth(961))

        let screen = CGRect(x: 0, y: 25, width: 1440, height: 875)
        let leftTiled = CGRect(x: 0, y: 25, width: 720, height: 875)
        let rightTiled = CGRect(x: 720, y: 25, width: 720, height: 875)
        let leftTiledWithMargin = CGRect(x: 12, y: 35, width: 708, height: 855)
        let fullScreen = CGRect(x: 0, y: 25, width: 1440, height: 875)
        let centeredWindow = CGRect(x: 200, y: 100, width: 1040, height: 700)

        #expect(PaneResizePolicy.isWindowTiledToSide(windowFrame: leftTiled, screenVisibleFrame: screen))
        #expect(PaneResizePolicy.isWindowTiledToSide(windowFrame: rightTiled, screenVisibleFrame: screen))
        #expect(PaneResizePolicy.isWindowTiledToSide(windowFrame: leftTiledWithMargin, screenVisibleFrame: screen))
        #expect(!PaneResizePolicy.isWindowTiledToSide(windowFrame: fullScreen, screenVisibleFrame: screen))
        #expect(!PaneResizePolicy.isWindowTiledToSide(windowFrame: centeredWindow, screenVisibleFrame: screen))

        // Large 2560x1440 display test
        let largeScreen = CGRect(x: 0, y: 25, width: 2560, height: 1415)
        let largeLeftTiled = CGRect(x: 0, y: 25, width: 1280, height: 1415)
        #expect(PaneResizePolicy.isWindowTiledToSide(windowFrame: largeLeftTiled, screenVisibleFrame: largeScreen))
        #expect(PaneResizePolicy.shouldCollapsePanesForTiling(containerWidth: 1280, windowFrame: largeLeftTiled, screenVisibleFrame: largeScreen))

        // Compact width collapses even without window frame
        #expect(PaneResizePolicy.shouldCollapsePanesForTiling(containerWidth: 800))
        // Wide centered window does not collapse
        #expect(!PaneResizePolicy.shouldCollapsePanesForTiling(containerWidth: 1200, windowFrame: centeredWindow, screenVisibleFrame: screen))
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
            .paragraph("説明の一行目\n説明の二行目"),
            .bullet("項目"),
            .quote("引用"),
            .separator,
            .code("ios", "interface vlan 20")
        ])
    }

    @Test func finalResponsePreservesLineBreaksThroughJSONAndInlineMarkdown() throws {
        let answer = """
        NakaokuGW のLAN1はupです。
        観測時刻: 2026-10-04T05:20:55Z
        根拠: Graphに保存・再取得したCanonicalインターフェース状態（status）。
        管理上の有効/無効（admin_state）と、IP疎通はこの確認では判定していません。
        """
        let decoded = try JSONDecoder().decode(String.self, from: JSONEncoder().encode(answer))
        let blocks = ChatMarkdownParser.parse(decoded)
        #expect(blocks.map(\.kind) == [.paragraph(answer)])
        let content = try #require(blocks.first)
        guard case let .paragraph(text) = content.kind else {
            Issue.record("Expected a paragraph for the final response")
            return
        }
        let attributed = try AttributedString(markdown: text, options: .init(interpretedSyntax: .inlineOnlyPreservingWhitespace))
        #expect(String(attributed.characters) == answer)
    }

    @Test func markdownPreservesNewlinesAlongsideInlineFormattingAndCode() throws {
        let blocks = ChatMarkdownParser.parse("**状態**: up\n観測時刻: 2026-10-04\n\n別の段落\n```text\n一行目\n二行目\n```\n文字通りの\\n")
        #expect(blocks.map(\.kind) == [
            .paragraph("**状態**: up\n観測時刻: 2026-10-04"),
            .paragraph("別の段落"),
            .code("text", "一行目\n二行目"),
            .paragraph("文字通りの\\n")
        ])
        let attributed = try AttributedString(markdown: "**状態**: up\n観測時刻: 2026-10-04", options: .init(interpretedSyntax: .inlineOnlyPreservingWhitespace))
        #expect(String(attributed.characters) == "状態: up\n観測時刻: 2026-10-04")
    }

    @MainActor @Test func clipboardCopiesJapaneseMarkdownAndReplacesExistingContents() {
        let pasteboard = TestClipboardBoard(contents: "古い内容")

        let message = "## VLAN 設定\n\n```ios\ninterface vlan 20\n description 管理用\n```"
        #expect(ChatMessageClipboard.copy(text: message, to: pasteboard))
        #expect(pasteboard.contents == message)
        #expect(pasteboard.clearCount == 1)
    }

    @Test func hoverScrollPolicyDetectsTruncationAndCalculatesOffsets() {
        // Fits within container
        #expect(!HoverScrollPolicy.isTruncated(textWidth: 100, containerWidth: 150))
        #expect(HoverScrollPolicy.maxScrollOffset(textWidth: 100, containerWidth: 150) == 0)
        #expect(HoverScrollPolicy.scrollDuration(overflow: 0) == 0)

        // Tolerance boundary
        #expect(!HoverScrollPolicy.isTruncated(textWidth: 150.4, containerWidth: 150))
        #expect(HoverScrollPolicy.isTruncated(textWidth: 150.6, containerWidth: 150))

        // Overflowing text
        #expect(HoverScrollPolicy.isTruncated(textWidth: 250, containerWidth: 200))
        #expect(HoverScrollPolicy.maxScrollOffset(textWidth: 250, containerWidth: 200) == -50)

        // Duration calculation with min clamp (35 / 70 = 0.5 -> clamped to 0.8)
        #expect(HoverScrollPolicy.scrollDuration(overflow: 35) == 0.8)

        // Duration calculation without clamp (140 / 70 = 2.0)
        #expect(HoverScrollPolicy.scrollDuration(overflow: 140) == 2.0)

        // Duration calculation with max clamp (420 / 70 = 6.0 -> clamped to 5.0)
        #expect(HoverScrollPolicy.scrollDuration(overflow: 420) == 5.0)

        // Zero or negative container width does not report truncated
        #expect(!HoverScrollPolicy.isTruncated(textWidth: 100, containerWidth: 0))
        #expect(!HoverScrollPolicy.isTruncated(textWidth: 100, containerWidth: -10))

        // System font text measurement
        let shortWidth = HoverScrollPolicy.textWidth(for: "短い")
        let longWidth = HoverScrollPolicy.textWidth(for: "非常に長いチャットセッションのタイトルです。F220のVLAN設定方法を詳しく教えてください。")
        #expect(longWidth > shortWidth)
        #expect(shortWidth > 0)
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
