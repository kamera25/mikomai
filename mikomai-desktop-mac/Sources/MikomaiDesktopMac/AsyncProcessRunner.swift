import Foundation

struct ProcessExecutionOutput: Sendable {
    let success: Bool
    let stdout: String
    let stderr: String
    let command: String?

    init(success: Bool, stdout: String, stderr: String, command: String? = nil) {
        self.success = success
        self.stdout = stdout
        self.stderr = stderr
        self.command = command
    }
}

enum AsyncProcessRunner {
    static func run(
        executableURL: URL,
        arguments: [String],
        environment: [String: String]? = nil,
        inputData: Data? = nil
    ) async -> ProcessExecutionOutput {
        await withCheckedContinuation { continuation in
            DispatchQueue.global(qos: .userInitiated).async {
                let process = Process()
                process.executableURL = executableURL
                process.arguments = arguments
                if let environment {
                    process.environment = environment
                }

                let outPipe = Pipe()
                let errPipe = Pipe()
                process.standardOutput = outPipe
                process.standardError = errPipe

                let inPipe: Pipe?
                if inputData != nil {
                    let p = Pipe()
                    process.standardInput = p
                    inPipe = p
                } else {
                    inPipe = nil
                }

                let commandText = ([executableURL.path] + arguments).joined(separator: " ")

                do {
                    try process.run()
                    if let inputData, let inPipe {
                        inPipe.fileHandleForWriting.write(inputData)
                        inPipe.fileHandleForWriting.closeFile()
                    }
                    let outData = outPipe.fileHandleForReading.readDataToEndOfFile()
                    let errData = errPipe.fileHandleForReading.readDataToEndOfFile()
                    process.waitUntilExit()

                    let stdout = String(decoding: outData, as: UTF8.self)
                    let stderr = String(decoding: errData, as: UTF8.self)
                    let output = ProcessExecutionOutput(
                        success: process.terminationStatus == 0,
                        stdout: stdout,
                        stderr: stderr,
                        command: commandText
                    )
                    continuation.resume(returning: output)
                } catch {
                    let output = ProcessExecutionOutput(
                        success: false,
                        stdout: "",
                        stderr: error.localizedDescription,
                        command: commandText
                    )
                    continuation.resume(returning: output)
                }
            }
        }
    }
}
