import Foundation
import Testing
@testable import MikomaiDesktopCore

@Suite
struct AttachmentMediaPolicyTests {
    @Test func imagePathsAreRecognizedByCaseInsensitiveExtension() {
        #expect(AttachmentMediaPolicy.isImagePath("router.PNG"))
        #expect(AttachmentMediaPolicy.isImagePath("captures/device.heic"))
        #expect(AttachmentMediaPolicy.isImagePath("preview.tiff"))
        #expect(!AttachmentMediaPolicy.isImagePath("router.conf"))
        #expect(!AttachmentMediaPolicy.isImagePath("router.png.txt"))
    }

    @Test func imageFileIsRecognizedByMimeOrExtension() {
        #expect(AttachmentMediaPolicy.isImageFile(name: "router.txt", mediaType: "image/png"))
        #expect(AttachmentMediaPolicy.isImageFile(name: "router.txt", mediaType: "image/svg+xml"))
        #expect(AttachmentMediaPolicy.isImageFile(name: "router.PNG", mediaType: "text/plain"))
        #expect(!AttachmentMediaPolicy.isImageFile(name: "router.conf", mediaType: "text/plain"))
    }

    @Test func nativeTextAttachmentPolicyRejectsImagesAndPDFs() {
        expectAttachmentError(.unsupportedType) {
            try TextAttachmentPolicy.prepare(name: "diagram.png", data: Data([0x89, 0x50]))
        }
        expectAttachmentError(.unsupportedType) {
            try TextAttachmentPolicy.prepare(name: "manual.pdf", data: Data("%PDF".utf8))
        }
    }
}

private func expectAttachmentError(
    _ expected: AttachmentReadError,
    operation: () throws -> PendingAttachment
) {
    do {
        _ = try operation()
        Issue.record("Expected attachment preparation to fail with \(expected)")
    } catch let error as AttachmentReadError {
        #expect(error == expected)
    } catch {
        Issue.record("Unexpected attachment error: \(error)")
    }
}
