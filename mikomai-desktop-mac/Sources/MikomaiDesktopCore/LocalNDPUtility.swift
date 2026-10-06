import Foundation

public struct LocalNDPOutput: Sendable {
    public let success: Bool
    public let stdout: String
    public let stderr: String
    public let exitCode: Int32?
    public let command = "/usr/sbin/ndp -a"
}

/// Run the fixed, read-only macOS neighbor-cache command in the host process.
public enum LocalNDPUtility {
    public static func read() -> LocalNDPOutput {
        do {
            let output: String = try JSONDecoder().decode(String.self,from:RustPolicy.data(["op":"native_tool","tool":"self_network_ndp","target":[:],"args":[:]]))
            return LocalNDPOutput(success:true,stdout:output,stderr:"",exitCode:0)
        } catch {return LocalNDPOutput(success:false,stdout:"",stderr:error.localizedDescription,exitCode:nil)}
    }
}
