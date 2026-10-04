import Foundation
import MikomaiFFI

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

    static func executeApprovedAgentOperation(planID: String, planHash: String, password: String?) async -> NetworkOperationOutput {
        let credentials: String
        do {
            credentials = String(decoding: try JSONSerialization.data(withJSONObject: ["password": password ?? ""]), as: UTF8.self)
        } catch {
            return NetworkOperationOutput(success: false, stdout: "", stderr: error.localizedDescription)
        }

        return await withCheckedContinuation { continuation in
            let completion = ApprovedOperationCompletion(continuation)
            let context = Unmanaged.passRetained(completion).toOpaque()
            let submission = planID.withCString { id in
                planHash.withCString { hash in
                    credentials.withCString { secretJSON in
                        call {
                            mikomai_operation_execute_approved_async(
                                id, hash, secretJSON, approvedOperationCompletionBridge, context
                            )
                        }
                    }
                }
            }
            // Failed submissions never invoke the callback. Successful ones
            // transfer context ownership to the callback, even if it fires
            // before this function returns from the FFI call.
            if !submission.isSuccess {
                Unmanaged<ApprovedOperationCompletion>.fromOpaque(context).release()
                continuation.resume(returning: operationOutput(submission))
            }
        }
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

private final class ApprovedOperationCompletion: Sendable {
    let continuation: CheckedContinuation<NetworkOperationOutput, Never>

    init(_ continuation: CheckedContinuation<NetworkOperationOutput, Never>) {
        self.continuation = continuation
    }
}

private func approvedOperationCompletionBridge(
    _ status: Int32, _ output: UnsafePointer<CChar>?, _ context: UnsafeMutableRawPointer?
) {
    guard let context else { return }
    let completion = Unmanaged<ApprovedOperationCompletion>.fromOpaque(context).takeRetainedValue()
    let result = FFIResult(status: status, message: output.map { String(cString: $0) } ?? "")
    completion.continuation.resume(returning: MikomaiFFIBridge.operationOutput(result))
}
