import Foundation

public struct LocalNDPOutput: Sendable {
    public let success: Bool
    public let stdout: String
    public let stderr: String
    public let exitCode: Int32?
    public let command = "/usr/sbin/ndp -a"
}

/// Run the fixed, read-only macOS neighbor-cache command in the host process.
public enum LocalNDPUtility {
    public static func read() -> LocalNDPOutput {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent("mikomai-ndp-\(UUID().uuidString)", isDirectory: true)
        let stdoutURL = directory.appendingPathComponent("stdout")
        let stderrURL = directory.appendingPathComponent("stderr")
        defer { try? FileManager.default.removeItem(at: directory) }
        do {
            try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
            try Data().write(to: stdoutURL)
            try Data().write(to: stderrURL)
            let stdoutHandle = try FileHandle(forWritingTo: stdoutURL)
            defer { try? stdoutHandle.close() }
            let stderrHandle = try FileHandle(forWritingTo: stderrURL)
            defer { try? stderrHandle.close() }
            let process = Process()
            process.executableURL = URL(fileURLWithPath: "/usr/sbin/ndp")
            process.arguments = ["-a"]
            // File output avoids waiting on a full pipe for a large cache.
            process.standardOutput = stdoutHandle
            process.standardError = stderrHandle
            try process.run()
            process.waitUntilExit()
            let stdout = try String(contentsOf: stdoutURL, encoding: .utf8)
            let stderr = try String(contentsOf: stderrURL, encoding: .utf8)
            return LocalNDPOutput(success: process.terminationStatus == 0, stdout: stdout, stderr: stderr, exitCode: process.terminationStatus)
        } catch {
            return LocalNDPOutput(success: false, stdout: "", stderr: "自機のNDP取得に失敗しました: \(error.localizedDescription)", exitCode: nil)
        }
    }
}
