import Foundation

private final class Capture {
    var evidence = ""
    var chunks = ""
    var done = false
    var calls = 0
}

private func routeTool(_ tool: UnsafePointer<CChar>?, _ target: UnsafePointer<CChar>?, _ args: UnsafePointer<CChar>?, _ output: UnsafeMutablePointer<CChar>?, _ capacity: UInt, _ context: UnsafeMutableRawPointer?) -> Int32 {
    guard let tool, let target, let args, let output, let context else { return -1 }
    precondition(String(cString: tool) == "self_network_route")
    let device = try! JSONSerialization.jsonObject(with: Data(String(cString: target).utf8)) as! [String: Any]
    precondition(device["hostname"] as? String == "localhost")
    let arguments = try! JSONSerialization.jsonObject(with: Data(String(cString: args).utf8)) as! [String: Any]
    let result = LocalRoutingUtility.read(scope: arguments["scope"] as? String ?? "default")
    let capture = Unmanaged<Capture>.fromOpaque(context).takeUnretainedValue()
    capture.calls += 1
    capture.evidence = result.stdout.trimmingCharacters(in: .whitespacesAndNewlines)
    let data = try! JSONSerialization.data(withJSONObject: ["success": result.success, "output": result.success ? result.stdout : result.stderr])
    guard data.count + 1 <= Int(capacity) else { return -1 }
    data.withUnsafeBytes { bytes in
        output.update(from: bytes.baseAddress!.assumingMemoryBound(to: CChar.self), count: data.count)
    }
    output[data.count] = 0
    return 0
}

private func stream(_ text: UnsafePointer<CChar>?, _ done: Int32, _ context: UnsafeMutableRawPointer?) {
    guard let text, let context else { return }
    let capture = Unmanaged<Capture>.fromOpaque(context).takeUnretainedValue()
    let chunk = String(cString: text)
    if chunk.hasPrefix("__MIKOMAI_DEBUG__") { return }
    capture.chunks += chunk
    capture.done = done != 0
}

@main
struct LocalRoutingChecks {
    static func main() {
        let invalid = LocalRoutingUtility.read(scope: "unsupported")
        precondition(!invalid.success && invalid.stderr.contains("defaultまたはtable"))
        for goal in ["localhost のデフォルトルートを教えて", "localhost のデフォルトルートはどこ？", "自機のルーティングを確認して"] {
            let capture = Capture()
            let context = Unmanaged.passUnretained(capture).toOpaque()
            let response = goal.withCString { message in
                "[]".withCString { devices in
                    let mode = mikomai_dispatch_mode(message, devices)
                    precondition(mode.status == 0 && String(cString: mode.message!) == "fast_router")
                    mikomai_result_free(mode)
                    return "".withCString { empty in
                        "/nonexistent-mikomai-routing-check".withCString { path in
                            mikomai_agent_chat_streaming(message, empty, path, path, empty, devices, stream, routeTool, nil, context)
                        }
                    }
                }
            }
            precondition(response.status == 0)
            let answer = String(cString: response.message!)
            mikomai_result_free(response)
            precondition(capture.calls == 1 && capture.done && capture.chunks == answer)
            precondition(!capture.evidence.isEmpty && answer.contains(capture.evidence))
            let expected = goal.contains("デフォルト") ? "自機のデフォルトルート" : "自機のルーティングテーブル"
            precondition(answer.hasPrefix(expected))
            if goal.contains("デフォルト") {
                precondition(answer.contains("デフォルトゲートウェイ:") && answer.contains("使用インターフェース:"))
            }
            let record = try! JSONSerialization.data(withJSONObject: ["query": goal, "status": 0, "text": answer], options: [.sortedKeys])
            print(String(decoding: record, as: UTF8.self))
        }
    }
}
