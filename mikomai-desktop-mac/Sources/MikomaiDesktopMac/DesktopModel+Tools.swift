import Foundation
import MikomaiDesktopCore
extension DesktopModel {
    nonisolated static func runPortableAgentTool(tool: String, target: PortableDeviceTarget, arguments: [String: Any]) -> NetworkOperationOutput {
        do {
            let data = try JSONSerialization.data(withJSONObject: ["op":"native_tool", "tool":tool, "target":["id":target.id ?? "", "hostname":target.hostname], "args":arguments])
            let text = try NativeCommands.request(String(decoding:data,as:UTF8.self))
            let output = try JSONDecoder().decode(String.self, from:Data(text.utf8))
            return NetworkOperationOutput(success:true,stdout:output,stderr:"")
        } catch { return NetworkOperationOutput(success:false,stdout:"",stderr:error.localizedDescription) }
    }
}
