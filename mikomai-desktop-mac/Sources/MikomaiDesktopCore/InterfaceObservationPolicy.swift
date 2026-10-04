import Foundation

/// Read-only Yamaha LAN MVP: never guess a port or interpolate arbitrary CLI text.
public enum InterfaceObservationPolicy {
    public static func command(deviceType: String, interface: String?) throws -> String {
        if DeviceTypeCatalog.canonicalID(for: deviceType) == "yamaha" {
            guard let name = interface?.lowercased(),
                  name.range(of: "^lan[1-9][0-9]{0,2}$", options: .regularExpression) != nil else {
                throw InterfaceObservationError.lanRequired
            }
            return "show status \(name)"
        }
        return "show interfaces"
    }
}

public enum InterfaceObservationError: LocalizedError {
    case lanRequired
    public var errorDescription: String? {
        "ヤマハルータの確認対象LAN名（例: LAN1）を1つ指定してください。"
    }
}
