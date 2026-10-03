import Foundation
import Testing
@testable import MikomaiDesktopCore

@Suite struct ChatDiagramTests {
    let svg = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"640\" height=\"320\"><text>ルータ</text></svg>"
    var source: String { "data:image/svg+xml;base64," + Data(svg.utf8).base64EncodedString() }

    @Test func separatesDiagramFromAnswerAndRestoresSavedMarkdown() throws {
        let text = "生成しました。\n\n![NW図](\(source))\n\nSVGを保存しました。"
        let blocks = ChatMarkdownParser.parse(text)
        #expect(blocks.map(\.kind) == [.paragraph("生成しました。"), .image("NW図", source), .paragraph("SVGを保存しました。")])
        let image = try #require(ChatDiagramImage(source: source))
        #expect(image.data == Data(svg.utf8))
        #expect(image.aspectRatio == 2)
        let saved = try JSONEncoder().encode(text)
        #expect(ChatMarkdownParser.parse(try JSONDecoder().decode(String.self, from: saved)) == blocks)
    }
    @Test func supportsViewBoxAndRejectsInvalidOrExternalImages() throws {
        let data = Data("<svg viewBox='0 0 300 600'><filter width='1.2' height='1.5'/></svg>".utf8)
        let image = try #require(ChatDiagramImage(source: "data:image/svg+xml;base64," + data.base64EncodedString()))
        #expect(image.aspectRatio == 0.5)
        #expect(ChatDiagramImage(source: "data:image/svg+xml;base64,invalid") == nil)
        #expect(ChatDiagramImage(source: "https://example.com/diagram.svg") == nil)
        #expect(ChatMarkdownParser.parse("```text\n![NW図](\(source))\n```").first?.kind == .code("text", "![NW図](\(source))"))
    }
}
