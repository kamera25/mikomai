import Foundation

public struct LocalRoutingOutput: Sendable {
    public let success: Bool
    public let stdout: String
    public let stderr: String
}

/// Read this Mac's routes. Answers are formatted in Japanese by mikomai-core.
public enum LocalRoutingUtility {
    public static func read(scope: String = "default", destination: String? = nil) -> LocalRoutingOutput {
        do {
            var args: [String:Any] = ["scope":scope]
            if let destination { args["destination"] = destination }
            let output: String = try JSONDecoder().decode(String.self,from:RustPolicy.data(["op":"native_tool","tool":"self_network_route","target":[:],"args":args]))
            return LocalRoutingOutput(success:true,stdout:output,stderr:"")
        } catch {return LocalRoutingOutput(success:false,stdout:"",stderr:error.localizedDescription)}
    }
}
