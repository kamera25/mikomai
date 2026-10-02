import Foundation
import SwiftUI
import MikomaiDesktopCore

@MainActor
final class OperationCoordinator: ObservableObject {
    private weak var model: DesktopModel?

    init(model: DesktopModel) {
        self.model = model
    }

    static func runnerDeviceType(_ value: String) -> String {
        let lower = value.lowercased()
        if lower.contains("juniper") { return "juniper_junos" }
        if lower.contains("nx-os") || lower.contains("nxos") { return "cisco_nxos" }
        if lower.contains("arista") { return "arista_eos" }
        if lower.contains("yamaha") { return "yamaha" }
        if lower.contains("furukawa") || lower.contains("fitel") { return "furukawa_fitelnet" }
        if lower.contains("cisco") { return "cisco_ios" }
        return lower.replacingOccurrences(of: " ", with: "_")
    }

    static func showConfigCommand(for connection: SavedConnection) -> String {
        let device = runnerDeviceType(connection.deviceType)
        if device == "juniper_junos" { return "show configuration" }
        if device == "yamaha" { return "show config" }
        return "show running-config"
    }

    nonisolated static func lineDiff(old: String, new: String) -> [String] {
        let oldLines = old.components(separatedBy: .newlines)
        let newLines = new.components(separatedBy: .newlines)
        let changes = newLines.difference(from: oldLines)
        let edits: [(Int, Int, String)] = changes.compactMap { change in
            switch change {
            case let .remove(offset, element, _): (offset, 0, "- \(element)")
            case let .insert(offset, element, _): (offset, 1, "+ \(element)")
            }
        }
        return edits.sorted { ($0.0, $0.1) < ($1.0, $1.1) }.map(\.2)
    }


    func preparePlan(
        connectionID: UUID,
        rationale: String
    ) async -> String? {
        guard let model, let connection = model.connections.first(where: { $0.id == connectionID }) else {
            return "対象機器が見つかりません。"
        }
        guard let request = model.networkRequest(
            action: "show",
            connection: connection,
            commands: [Self.showConfigCommand(for: connection)]
        ) else {
            return "現在、Console 接続の変更計画には対応していません。SSH 接続の機器を選んでください。"
        }

        model.operationLogs.append("[STATUS] 1/4 現状のConfigを取得中")
        model.operationPhase = "現状のConfigを取得中…"
        let output = await Task.detached { DesktopModel.runNetworkWrapper(request) }.value
        if !output.stderr.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            model.operationLogs.append(contentsOf: output.stderr.split(whereSeparator: \.isNewline).map(String.init))
        }

        guard output.success, !output.stdout.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
              !NetworkCommandOutputPolicy.hasError(in: output.stdout) else {
            model.operationLogs.append("[ERROR] 現状Config取得に失敗しました。変更計画は作成していません。")
            model.operationPhase = "現状取得失敗"
            return "現状のConfigを取得できませんでした。機器情報と接続を確認してください。"
        }

        model.operationBeforeConfig = output.stdout
        if let error = model.createOperationPlan(target: connection, proposal: model.operationProposal, rationale: rationale) {
            model.operationPhase = "計画作成失敗"
            return error
        }

        model.operationLogs.append("[STATUS] 現状取得後、機器・コマンドに固定した変更計画を作成しました")
        return nil
    }

    func approveAndExecutePlan() async -> String? {
        guard let model, let plan = model.operationPlan,
              plan.status == "pending", let (connection, credentials) = model.resolveOperationTarget(for: plan) else {
            return "計画作成後に対象機器の情報が変わりました。変更計画を作り直してください。"
        }

        guard model.approveOperationPlan() == nil, model.beginOperationPlan() == nil else {
            model.operationLogs.append("[ERROR] ハッシュ照合による承認に失敗しました")
            model.operationPhase = "承認失敗"
            return "変更計画を承認できませんでした。"
        }

        if plan.toolId != "network_config" {
            model.operationPhase = "承認済み操作を実行中…"
            model.operationLogs.append("[STATUS] 承認済み操作を実行中")
            let output = await Task.detached {
                DesktopModel.executeApprovedAgentOperation(planID: plan.id, planHash: plan.planHash, password: credentials.password)
            }.value
            model.operationLogs.append(contentsOf: output.stdout.split(whereSeparator: \.isNewline).map(String.init))
            if !output.stderr.isEmpty {
                model.operationLogs.append(contentsOf: output.stderr.split(whereSeparator: \.isNewline).map(String.init))
            }
            model.finishOperationPlan(succeeded: output.success)
            model.operationPhase = output.success ? "承認済み操作が完了しました" : "承認済み操作が失敗しました"
            return output.success ? nil : "操作に失敗しました。ログを確認してください。"
        }

        let planCommands = plan.args.commands ?? []
        guard !planCommands.isEmpty else {
            model.finishOperationPlan(succeeded: false)
            model.operationPhase = "実行経路未接続"
            return "この操作はSwift側の承認済み実行経路がまだ接続されていません。"
        }

        let target = plan.args.deviceSnapshot
        let approvedRequest = NetworkRunnerRequest(
            action: "dry_run", host: target.host, username: target.username,
            password: credentials.password ?? "", secret: credentials.enablePassword ?? "",
            deviceType: connection.transportDeviceType(Self.runnerDeviceType(target.deviceType)),
            port: connection.effectivePort, commands: planCommands
        )

        model.operationPhase = "2/4 dry-run 検証中…"
        model.operationLogs.append("[STATUS] 2/4 dry-run 検証中")

        let configRequest = NetworkRunnerRequest(
            action: "config", host: target.host, username: target.username,
            password: credentials.password ?? "", secret: credentials.enablePassword ?? "",
            deviceType: connection.transportDeviceType(Self.runnerDeviceType(target.deviceType)),
            port: connection.effectivePort, commands: planCommands
        )

        let workflow = await OperationWorkflow.execute(
            dryRun: {
                let output = await Task.detached { DesktopModel.runNetworkWrapper(approvedRequest) }.value
                return OperationCommandOutput(processSucceeded: output.success, stdout: output.stdout, stderr: output.stderr)
            },
            configure: {
                await MainActor.run {
                    model.operationPhase = "3/4 Config 投入中…"
                    model.operationLogs.append("[STATUS] 3/4 Configを投入中")
                }
                let output = await Task.detached { DesktopModel.runNetworkWrapper(configRequest) }.value
                return OperationCommandOutput(processSucceeded: output.success, stdout: output.stdout, stderr: output.stderr)
            }
        )

        let dryRun = workflow.dryRun
        model.operationLogs.append(contentsOf: dryRun.stderr.split(whereSeparator: \.isNewline).map(String.init))
        guard workflow.dryRunPassed, let deployed = workflow.configuration else {
            model.operationLogs.append("[ERROR] dry-runに失敗したためConfig投入を中止しました")
            model.operationPhase = "dry-run失敗"
            model.finishOperationPlan(succeeded: false)
            return "dry-runでエラーが見つかったため、機器への投入を中止しました。"
        }

        model.operationLogs.append(contentsOf: deployed.stderr.split(whereSeparator: \.isNewline).map(String.init))
        guard deployed.processSucceeded else {
            model.operationLogs.append("[ERROR] Config投入に失敗しました")
            model.operationPhase = "投入失敗"
            model.finishOperationPlan(succeeded: false)
            return "Configを投入できませんでした。ログを確認してください。"
        }

        model.operationPhase = "4/4 投入後のConfigを検証中…"
        model.operationLogs.append("[STATUS] 4/4 投入後のConfigを取得して差分を検証中")
        let verifyRequest = NetworkRunnerRequest(
            action: "show", host: target.host, username: target.username,
            password: credentials.password ?? "", secret: credentials.enablePassword ?? "",
            deviceType: connection.transportDeviceType(Self.runnerDeviceType(target.deviceType)),
            port: connection.effectivePort,
            commands: [Self.showConfigCommand(for: connection)]
        )
        let verified = await Task.detached { DesktopModel.runNetworkWrapper(verifyRequest) }.value
        model.operationLogs.append(contentsOf: verified.stderr.split(whereSeparator: \.isNewline).map(String.init))
        guard verified.success, !verified.stdout.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
              !NetworkCommandOutputPolicy.hasError(in: verified.stdout) else {
            model.operationLogs.append("[ERROR] 投入後Configの取得に失敗しました")
            model.operationPhase = "検証失敗"
            model.finishOperationPlan(succeeded: false)
            return "Configは投入されましたが、投入後の状態を確認できませんでした。"
        }

        let before = model.operationBeforeConfig
        let after = verified.stdout
        let diff = await Task.detached { Self.lineDiff(old: before, new: after) }.value
        model.operationAfterConfig = after
        model.operationDiffLines = diff
        model.finishOperationPlan(succeeded: true)
        model.operationPhase = "投入後Configを取得しました。差分を確認してください"
        model.operationLogs.append("[STATUS] Config投入が成功し、投入後Configを取得しました。差分を確認してください")
        return nil
    }
}
