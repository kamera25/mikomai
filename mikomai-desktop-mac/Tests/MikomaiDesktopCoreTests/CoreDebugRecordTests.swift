import Foundation
import Testing
@testable import MikomaiDesktopCore

struct CoreDebugRecordTests {
    @Test func searchFindsJapaneseEscapedJSONAndIgnoresCase() {
        let record = CoreDebugRecord(json: #"{"kind":"llm_request","payload":{"prompt":"\u65e5\u672c\u8a9e VLAN"}}"#)
        #expect(record.matches("日本語"))
        #expect(record.matches("vlan"))
        #expect(record.matches(""))
        #expect(!record.matches("missing"))
    }

    @Test func exportIsCompleteJSONLinesWithRoundTripContent() throws {
        let prompt = "日本語\n\"quoted\"\u{0000}text"
        let records = [
            CoreDebugRecord(json: CoreDebugRecord.encode(kind: "llm_request", payload: ["prompt":prompt])),
            CoreDebugRecord(json: CoreDebugRecord.encode(kind: "llm_response", payload: ["answer":"ok"]))
        ]
        let exported = CoreDebugRecord.export(records)
        let lines = exported.split(separator: "\n")
        #expect(lines.count == 2)
        let object = try #require(JSONSerialization.jsonObject(with: Data(lines[0].utf8)) as? [String: Any])
        let payload = try #require(object["payload"] as? [String: String])
        #expect(payload["prompt"] == prompt)
        #expect(exported.hasSuffix("\n"))
        #expect(CoreDebugRecord.export([]).isEmpty)
    }
}
