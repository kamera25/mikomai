import Foundation
import SwiftUI
import MikomaiDesktopCore
import MikomaiBindings

@MainActor
final class OperationCoordinator: ObservableObject {
    private weak var model: DesktopModel?
    init(model: DesktopModel) { self.model = model }
    func preparePlan(connectionID: UUID, rationale: String) async -> String? {
        guard let model, let connection = model.connections.first(where: { $0.id == connectionID }) else { return "対象機器が見つかりません。" }
        return model.createOperationPlan(target: connection, proposal: model.operationProposal, rationale: rationale)
    }
    func approveAndExecutePlan() async -> String? {
        guard let model, let plan = model.operationPlan else { return "変更計画がありません。" }
        if let error = model.approveOperationPlan() { return error }
        model.operationPhase = "承認済み操作を実行中…"
        let service=MikomaiService()
        do {
            let id=try service.submit(command:.executeApproved(planId:plan.id,planHash:plan.planHash))
            model.operationTaskID=id
            defer {model.operationTaskID=nil;model.operationWaitingDecision=false}
            while true {
                let snapshot=try service.query(query:.task(taskId:id))
                model.operationWaitingDecision=snapshot.state == "awaiting_user"
                if model.operationWaitingDecision {model.operationPhase="機器のロック待ちが続いています。継続か中止を選んでください。"}
                if ["completed","failed","cancelled","unknown"].contains(snapshot.state) {
                    model.operationLogs.append(snapshot.result)
                    let result=legacyInvoke(op:"mikomai_operation_plan_get",args:[plan.id],listener:nil)
                    if result.status == 0 {model.operationPlan=try? JSONDecoder().decode(NativeOperationPlan.self,from:Data(result.text.utf8))}
                    if snapshot.state == "completed" {
                        if let object=try? JSONSerialization.jsonObject(with:Data(snapshot.result.utf8)) as? [String:Any] {
                            model.operationBeforeConfig=object["before_config"] as? String ?? ""
                            model.operationAfterConfig=object["after_config"] as? String ?? ""
                            model.operationDiffLines=object["diff"] as? [String] ?? []
                        }
                        model.operationPhase="操作完了・差分を確認";return nil
                    }
                    model.operationPhase=snapshot.state == "unknown" ? "結果不明・実機を確認してください" : "操作を完了できませんでした"
                    return snapshot.result.isEmpty ? "操作を中止しました。" : snapshot.result
                }
                try await Task.sleep(for:.milliseconds(20))
            }
        } catch {return error.localizedDescription}
    }
}
