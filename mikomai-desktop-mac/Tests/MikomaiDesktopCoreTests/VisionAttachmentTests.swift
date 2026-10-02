import Foundation
import Testing
@testable import MikomaiDesktopCore

@Suite struct VisionAttachmentTests {
    let png = Data([0x89,0x50,0x4e,0x47,0x0d,0x0a,0x1a,0x0a])
    @Test func imageAndTextUseVersionedPayloadWithoutLosingBytes() throws {
        let image = try ImageAttachmentPolicy.prepare(name: "diagram.png", data: png, existing: [], visionEnabled: true)
        let text = PendingAttachment(name: "note.md", text: "メモ")
        let encoded = try NativeAttachmentPayload.encode([image, text])
        #expect(encoded.hasPrefix("__MIKOMAI_ATTACHMENTS_V1__"))
        let json = Data(encoded.dropFirst("__MIKOMAI_ATTACHMENTS_V1__".count).utf8)
        let payload = try #require(JSONSerialization.jsonObject(with: json) as? [String: Any])
        let images = try #require(payload["images"] as? [[String: String]])
        #expect(Data(base64Encoded: images[0]["base64"] ?? "") == png)
        #expect(images[0]["mimeType"] == "image/png")
        #expect((payload["text"] as? String)?.contains("メモ") == true)
        #expect(image.byteCount == png.count)
    }
    @Test func disabledVisionAndFakeImageAreRejected() {
        #expect(throws: ImageAttachmentError.self) {
            try ImageAttachmentPolicy.prepare(name: "diagram.png", data: png, existing: [], visionEnabled: false)
        }
        #expect(throws: ImageAttachmentError.self) {
            try ImageAttachmentPolicy.prepare(name: "diagram.png", data: Data("fake".utf8), existing: [], visionEnabled: true)
        }
    }
    @Test func textOnlyKeepsExistingWireFormat() throws {
        #expect(try NativeAttachmentPayload.encode([PendingAttachment(name: "note.md", text: "メモ")]) == "[添付ファイル 1: note.md]\nメモ")
    }
    @Test func imageCountAndByteLimitsAreEnforced() throws {
        let existing = (0..<4).map { PendingAttachment(name: "\($0).png", text: "", imageData: png, mimeType: "image/png") }
        #expect(throws: ImageAttachmentError.self) {
            try ImageAttachmentPolicy.prepare(name: "fifth.png", data: png, existing: existing, visionEnabled: true)
        }
        #expect(throws: ImageAttachmentError.self) {
            try ImageAttachmentPolicy.prepare(name: "big.png", data: Data(repeating: 0, count: ImageAttachmentPolicy.maxFileBytes + 1), existing: [], visionEnabled: true)
        }
    }
}
