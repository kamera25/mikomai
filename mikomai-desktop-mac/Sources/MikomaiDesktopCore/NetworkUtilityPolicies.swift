import Foundation
public enum IPAddressPolicy {public static func isGlobalIP(_ value:String)->Bool {RustPolicy.call(["op":"public_ip","value":value])}}
public struct PingCommand:Equatable,Codable {
    public var host:String;public var size:Int?;public var count:Int?;public var df:Bool?
    public init(host:String,size:Int?=nil,count:Int?=nil,df:Bool?=nil){self.host=host;self.size=size;self.count=count;self.df=df}
    public var processArguments:[String]?{RustPolicy.call(["op":"ping_arguments","command":RustPolicy.object(self)])}
}
public enum PingCommandParser {public static func parse(_ input:String)->PingCommand?{RustPolicy.call(["op":"ping_parse","value":input])}}
public enum CPUUsagePolicy {
    public static func command(for deviceType:String)->String{RustPolicy.call(["op":"cpu_command","value":deviceType])}
    public static func parse(_ output:String)->Double?{RustPolicy.call(["op":"cpu_usage","value":output])}
}
public enum NetworkDeviceError: Error, Equatable, Sendable {
    case invalidInput(String)
    case incompleteCommand(String)
    case ambiguousCommand(String)
    case syntaxError(String)
    case netmikoError(String)
    case deviceError(String)

    public var localizedDescription: String {
        switch self {
        case .invalidInput(let detail): return "無効なコマンド入力: \(detail)"
        case .incompleteCommand(let detail): return "不完全なコマンド: \(detail)"
        case .ambiguousCommand(let detail): return "曖昧なコマンド: \(detail)"
        case .syntaxError(let detail): return "構文エラー: \(detail)"
        case .netmikoError(let detail): return "Netmiko 実行エラー: \(detail)"
        case .deviceError(let detail): return "機器エラー: \(detail)"
        }
    }
}


public enum NetworkCommandOutputPolicy {
    private struct ErrorDTO:Decodable {let kind:String;let detail:String}
    public static func detectError(in output:String)->NetworkDeviceError?{
        guard let value:ErrorDTO=RustPolicy.call(["op":"network_error","value":output],as:ErrorDTO?.self) else{return nil}
        switch value.kind{
        case "invalidInput":return .invalidInput(value.detail)
        case "incompleteCommand":return .incompleteCommand(value.detail)
        case "ambiguousCommand":return .ambiguousCommand(value.detail)
        case "syntaxError":return .syntaxError(value.detail)
        case "netmikoError":return .netmikoError(value.detail)
        default:return .deviceError(value.detail)
        }
    }
    public static func hasError(in output:String)->Bool{detectError(in:output) != nil}
}
public enum ARPCommandPolicy {public static func command(for deviceType:String)->String{RustPolicy.call(["op":"arp_command","value":deviceType])}}
public enum RegisteredDiagnosticHostPolicy {
    private struct ResultDTO:Decodable{let host:String?;let error:String?}
    public static func resolve(_ host:String,connections:[SavedConnection])throws->String{
        let value:ResultDTO=RustPolicy.call(["op":"diagnostic_host","host":host,"connections":RustPolicy.object(connections)])
        if let host=value.host{return host}
        switch value.error {case "missingAddress":throw ResolutionError.missingAddress;case "invalidAddress":throw ResolutionError.invalidAddress;default:throw ResolutionError.ambiguous}
    }
    public enum ResolutionError:LocalizedError {
        case ambiguous,missingAddress,invalidAddress
        public var errorDescription:String?{switch self{
        case .ambiguous:return "同じ名前の登録機器が複数あります。対象のIPアドレスを指定してください。"
        case .missingAddress:return "登録機器の接続先が未設定です。IPアドレスを設定してください。"
        case .invalidAddress:return "登録機器のホスト欄に有効なIPアドレスがありません。名前解決を避けるため、IPアドレスを登録してください。"
        }}
    }
}
