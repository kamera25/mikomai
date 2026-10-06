import Foundation

public struct OperationCommandOutput: Sendable {
    public let processSucceeded: Bool
    public let stdout: String
    public let stderr: String

    public init(processSucceeded: Bool, stdout: String, stderr: String = "") {
        self.processSucceeded = processSucceeded
        self.stdout = stdout
        self.stderr = stderr
    }
}

public struct OperationWorkflowResult: Sendable {
    public let dryRun: OperationCommandOutput
    public let configuration: OperationCommandOutput?
    public var dryRunPassed: Bool { configuration != nil }
}

/// Enforces the safety boundary between validation and device configuration.
public enum OperationWorkflow {
    public static func acceptsDryRun(processSucceeded: Bool, json: String) -> Bool {
        RustPolicy.call(["op": "dry_run", "processSucceeded": processSucceeded, "output": json])
    }
}
