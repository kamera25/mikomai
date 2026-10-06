import Foundation
import Testing
import MikomaiBindings
@Suite(.serialized, .enabled(if: ProcessInfo.processInfo.environment["MIKOMAI_GRAPH_DB_PATH"]?.contains("mikomai-test-") == true))
struct TaskContractTests {
    final class Listener: EventListener, @unchecked Sendable {
        let lock=NSLock();var events:[TaskEvent]=[]
        func onEvent(event:TaskEvent) {lock.lock();events.append(event);lock.unlock()}
        func received(id:String)->[UInt64] {lock.lock();defer{lock.unlock()};return events.filter{$0.taskId==id}.map(\.seq)}
    }
    @Test func sameFixtureMatchesCallbacksSnapshotsAndCLIContract() throws {
        let root=URL(fileURLWithPath:#filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        let fixture=try String(contentsOf:root.appendingPathComponent("contracts/fixtures/task-lifecycle.json"),encoding:.utf8)
        let expected=try JSONSerialization.jsonObject(with:Data(contentsOf:root.appendingPathComponent("contracts/fixtures/task-lifecycle.expected.json"))) as! [String:Any]
        let service=MikomaiService();let listener=Listener();service.subscribe(listener:listener)
        let id=try service.submit(command:.contract(fixtureJson:fixture))
        let deadline=Date().addingTimeInterval(20)
        var snapshot=try service.query(query:.task(taskId:id))
        while !["completed","failed"].contains(snapshot.state) && Date()<deadline {Thread.sleep(forTimeInterval:0.005);snapshot=try service.query(query:.task(taskId:id))}
        #expect(snapshot.state=="completed")
        #expect(snapshot.result==expected["result"] as? String)
        #expect(snapshot.events.map(\.kind)==expected["kinds"] as? [String])
        #expect(snapshot.events.map(\.seq)==Array(1...UInt64(snapshot.events.count)))
        #expect(snapshot.events.allSatisfy{$0.version==1 && $0.taskId==id})
        #expect(listener.received(id:id)==snapshot.events.map(\.seq))
    }
}
