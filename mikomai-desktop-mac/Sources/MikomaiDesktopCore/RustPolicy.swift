import Foundation
import MikomaiFFI

/// Temporary DTO transport; all policy decisions are made in mikomai-app.
enum RustPolicy {
    static func object<T: Encodable>(_ value: T) -> Any {
        do { return try JSONSerialization.jsonObject(with: JSONEncoder().encode(value), options: [.fragmentsAllowed]) }
        catch { preconditionFailure("Cannot encode native DTO: \(error)") }
    }

    static func call<T: Decodable>(_ request: [String: Any], as: T.Type = T.self) -> T {
        do {
            let data = try JSONSerialization.data(withJSONObject: request, options: [.fragmentsAllowed])
            let result = String(decoding: data, as: UTF8.self).withCString { mikomai_native_query($0) }
            defer { mikomai_result_free(result) }
            guard let message = result.message else { preconditionFailure("Native policy returned no result") }
            let text = String(cString: message)
            guard result.status == 0 else { preconditionFailure("Native policy failed: \(text)") }
            return try JSONDecoder().decode(T.self, from: Data(text.utf8))
        } catch { preconditionFailure("Invalid native policy response: \(error)") }
    }
}

public enum LegacyDataNotice {
    public static var existingDirectories: [String] { RustPolicy.call(["op": "legacy_locations"]) }
}
