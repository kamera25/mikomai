import Foundation

public struct CoreDebugRecord: Identifiable, Sendable {
    public let id = UUID()
    public let json: String
    public init(json: String) { self.json = json }
    public func matches(_ query: String) -> Bool {
        query.isEmpty || json.localizedCaseInsensitiveContains(query) || formatted.localizedCaseInsensitiveContains(query)
    }
    public static func export(_ records: [CoreDebugRecord]) -> String {
        records.isEmpty ? "" : records.map(\.json).joined(separator: "\n") + "\n"
    }
    public var formatted: String {
        guard let data = json.data(using: .utf8),
              let object = try? JSONSerialization.jsonObject(with: data),
              let pretty = try? JSONSerialization.data(withJSONObject: object, options: [.prettyPrinted, .sortedKeys, .fragmentsAllowed]) else { return json }
        return String(decoding: pretty, as: UTF8.self)
    }
    public static func encode(kind: String, payload: [String: Any]) -> String {
        let record: [String: Any] = ["timestamp": ISO8601DateFormatter().string(from: Date()), "kind": kind, "payload": payload]
        return String(decoding: (try? JSONSerialization.data(withJSONObject: record, options: [.sortedKeys])) ?? Data(), as: UTF8.self)
    }
}

