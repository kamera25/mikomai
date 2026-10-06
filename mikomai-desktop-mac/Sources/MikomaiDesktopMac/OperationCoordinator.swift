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
        let output = await MikomaiFFIBridge.executeApprovedAgentOperation(planID: plan.id, planHash: plan.planHash)
        model.operationLogs.append(output.success ? output.stdout : output.stderr)
        let response = plan.id.withCString { pointer in MikomaiFFIBridge.call { mikomai_operation_plan_get(pointer) } }
        if response.status == 0 { model.operationPlan = try? JSONDecoder().decode(NativeOperationPlan.self, from: Data(response.message.utf8)) }
        model.operationPhase = output.success ? "操作完了" : "操作結果を確認してください"
        return output.success ? nil : output.stderr
    }
}
