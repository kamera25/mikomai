import Foundation

/// Read-only Yamaha LAN MVP: never guess a port or interpolate arbitrary CLI text.
public enum InterfaceObservationPolicy {
    public static func command(deviceType: String, interface: String?) throws -> String {
        var request:[String:Any]=["op":"interface_command","deviceType":deviceType]
        if let interface {request["interface"]=interface}
        do {return try JSONDecoder().decode(String.self,from:RustPolicy.data(request))}
        catch {throw InterfaceObservationError.lanRequired}
    }
}

public enum InterfaceObservationError: LocalizedError {
    case lanRequired
    public var errorDescription: String? {
        "ヤマハルータの確認対象LAN名（例: LAN1）を1つ指定してください。"
    }
}
