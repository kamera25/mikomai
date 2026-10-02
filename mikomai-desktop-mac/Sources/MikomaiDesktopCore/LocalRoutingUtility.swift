import Foundation

public struct LocalRoutingOutput: Sendable {
    public let success: Bool
    public let stdout: String
    public let stderr: String
}

/// Read this Mac's routes. Answers are formatted in Japanese by mikomai-core.
public enum LocalRoutingUtility {
    public static func read(scope: String = "default") -> LocalRoutingOutput {
        let executable: String
        let arguments: [String]
        switch scope {
        case "default":
            executable = "/sbin/route"
            arguments = ["-n", "get", "default"]
        case "table":
            executable = "/usr/sbin/netstat"
            arguments = ["-rn"]
        default:
            return LocalRoutingOutput(success: false, stdout: "", stderr: "経路の取得範囲はdefaultまたはtableを指定してください。")
        }
        let process = Process()
        process.executableURL = URL(fileURLWithPath: executable)
        process.arguments = arguments
        let output = Pipe()
        let error = Pipe()
        process.standardOutput = output
        process.standardError = error
        do {
            try process.run()
            // Drain large routing tables before waiting for the child to exit.
            let stdout = String(decoding: output.fileHandleForReading.readDataToEndOfFile(), as: UTF8.self)
            let stderr = String(decoding: error.fileHandleForReading.readDataToEndOfFile(), as: UTF8.self)
            process.waitUntilExit()
            let success = process.terminationStatus == 0 && !stdout.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
            return LocalRoutingOutput(success: success, stdout: stdout, stderr: success ? stderr : (stderr.isEmpty ? "自機の経路情報を取得できませんでした。" : stderr))
        } catch {
            return LocalRoutingOutput(success: false, stdout: "", stderr: "自機の経路の取得に失敗しました: \(error.localizedDescription)")
        }
    }
}
