import Testing
@testable import MikomaiDesktopCore

@Suite
struct FilePathPolicyTests {
    @Test func extractsFilenameFromWindowsAndPosixPaths() {
        #expect(FilePathPolicy.defaultFilename("C:\\tmp\\router.txt") == "router.txt")
        #expect(FilePathPolicy.defaultFilename("/tmp/router.txt") == "router.txt")
    }
}
