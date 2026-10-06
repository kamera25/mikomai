import Foundation

public enum DeviceTypeCatalog {
    private struct Catalog: Decodable { let deviceTypes: [String]; let aliases: [String: String] }
    private static let catalog: Catalog = RustPolicy.call(["op": "device_catalog"])
    public static var deviceTypes: [String] { catalog.deviceTypes }
    public static var aliases: [String: String] { catalog.aliases }
    public static func canonicalID(for value: String) -> String {
        RustPolicy.call(["op": "device_id", "value": value])
    }
    public static func displayName(for value: String) -> String { aliases[canonicalID(for: value)] ?? value }
    public static func optionLabel(for value: String) -> String {
        let name = displayName(for: value)
        return name == value ? value : "\(name) (\(value))"
    }
    public static func matching(_ query: String) -> [String] {
        let query = query.trimmingCharacters(in: .whitespacesAndNewlines)
        return deviceTypes.filter { query.isEmpty || $0.localizedCaseInsensitiveContains(query)
            || displayName(for: $0).localizedCaseInsensitiveContains(query) }
    }
}
