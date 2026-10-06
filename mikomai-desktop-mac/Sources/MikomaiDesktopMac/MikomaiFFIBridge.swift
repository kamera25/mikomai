import Foundation
import MikomaiBindings

struct FFIResult: Sendable {
    let status: Int32
    let message: String

    var isSuccess: Bool { status == 0 }

    var formattedOutput: String {
        isSuccess ? message : "エラー: \(message)"
    }
}

enum MikomaiFFIBridge {
    static func call(_ operation: () -> MikomaiResult) -> FFIResult {
        let raw = operation()
        defer { mikomai_result_free(raw) }
        let message = raw.message.map { String(cString: $0) } ?? (raw.status == 0 ? "" : "応答がありませんでした。")
        return FFIResult(status: raw.status, message: message)
    }

    static func testTCP(host: String, port: UInt16, timeoutMs: UInt32) -> (success: Bool, message: String, latencyMs: Int?) {
        let result = call {
            host.withCString { cHost in
                mikomai_test_tcp_connection(cHost, port, timeoutMs)
            }
        }
        let text = result.message
        let latency: Int? = {
            if let msRange = text.range(of: "ms") {
                let prefix = text[..<msRange.lowerBound].trimmingCharacters(in: .whitespaces)
                if let lastSep = prefix.lastIndex(where: { $0 == " " || $0 == "," }) {
                    let numPart = prefix[prefix.index(after: lastSep)...]
                    return Int(numPart)
                }
            }
            return nil
        }()
        return (result.isSuccess, text, latency)
    }

    static func dispatchMode(_ prompt: String, devicesJSON: String) -> String {
        prompt.withCString { message in
            devicesJSON.withCString { targets in
                let result = call { mikomai_dispatch_mode(message, targets) }
                return result.message.isEmpty ? "worker" : result.message
            }
        }
    }

    static func loadModel(path: String) -> FFIResult {
        path.withCString { cPath in
            call { mikomai_model_load(cPath) }
        }
    }

    static func modelStatus() -> String {
        let res = call { mikomai_model_status() }
        return res.message
    }

    static func cancelModel() -> FFIResult {
        call { mikomai_model_cancel() }
    }

    static func setInferenceParams(temperature: Float, repetitionPenalty: Float, nCtx: UInt32, maxGen: UInt32) -> FFIResult {
        call { mikomai_set_inference_params(temperature, repetitionPenalty, nCtx, maxGen) }
    }

    static func executeApprovedAgentOperation(planID: String, planHash: String) async -> NetworkOperationOutput {
        let response = await Task.detached { legacyInvoke(op:"mikomai_operation_execute_approved",args:[planID,planHash],listener:nil) }.value
        return operationOutput(FFIResult(status:response.status,message:response.text))
    }

    fileprivate static func operationOutput(_ res: FFIResult) -> NetworkOperationOutput {
        guard res.isSuccess else {
            return NetworkOperationOutput(success: false, stdout: "", stderr: res.message.isEmpty ? "承認済み操作が失敗しました。" : res.message)
        }

        let text = res.message
        if let data = text.data(using: .utf8), let payload = try? JSONSerialization.jsonObject(with: data) as? [String: Any] {
            let output = payload["output"] as? String ?? text
            return NetworkOperationOutput(success: true, stdout: output, stderr: "")
        }
        return NetworkOperationOutput(success: true, stdout: text, stderr: "")
    }
}
