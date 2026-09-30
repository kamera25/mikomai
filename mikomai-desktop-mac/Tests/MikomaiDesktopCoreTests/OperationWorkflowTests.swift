import Testing
@testable import MikomaiDesktopCore

@Suite
struct OperationWorkflowTests {
    @Test func rejectedDryRunNeverCallsConfigRunner() async {
        for payload in [
            #"{"success":false,"results":[{"ok":true}]}"#,
            #"{"success":true,"results":[{"ok":false}]}"#
        ] {
            let calls = WorkflowCallRecorder()
            let result = await OperationWorkflow.execute(dryRun: {
                await calls.record("dry-run")
                return OperationCommandOutput(processSucceeded: true, stdout: payload)
            }, configure: {
                await calls.record("config")
                return OperationCommandOutput(processSucceeded: true, stdout: "")
            })
            #expect(!result.dryRunPassed)
            #expect(await calls.values == ["dry-run"])
        }
    }

    @Test func acceptedDryRunRunsConfigExactlyOnceAfterValidation() async {
        let calls = WorkflowCallRecorder()
        let result = await OperationWorkflow.execute(dryRun: {
            await calls.record("dry-run")
            return OperationCommandOutput(processSucceeded: true, stdout: #"{"success":true,"results":[{"ok":true}]}"#)
        }, configure: {
            await calls.record("config")
            return OperationCommandOutput(processSucceeded: true, stdout: "applied")
        })
        #expect(result.dryRunPassed)
        #expect(result.configuration?.stdout == "applied")
        #expect(await calls.values == ["dry-run", "config"])
    }
}

private actor WorkflowCallRecorder {
    private(set) var values: [String] = []
    func record(_ value: String) { values.append(value) }
}
