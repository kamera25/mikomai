import Testing
@testable import MikomaiDesktopCore
@Suite struct OperationWorkflowTests {
    @Test func rustRejectsFailedAndMalformedValidation() {
        #expect(!OperationWorkflow.acceptsDryRun(processSucceeded:false,json:"{}"))
        #expect(!OperationWorkflow.acceptsDryRun(processSucceeded:true,json:"garbage"))
        #expect(!OperationWorkflow.acceptsDryRun(processSucceeded:true,json:"{\"results\":[]}"))
        #expect(!OperationWorkflow.acceptsDryRun(processSucceeded:true,json:"{\"results\":[{\"ok\":false}]}"))
        #expect(OperationWorkflow.acceptsDryRun(processSucceeded:true,json:"{\"success\":true,\"results\":[{\"ok\":true}]}"))
    }
}
