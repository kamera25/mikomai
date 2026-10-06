import Foundation
import MikomaiFFI

public enum NativeStoreError: LocalizedError {
    case failure(String)
    public var errorDescription: String? {
        switch self { case .failure(let message): return message }
    }
}

/// Temporary DTO transport; all policy decisions are made in mikomai-app.
enum RustPolicy {
    static func object<T: Encodable>(_ value: T) -> Any {
        do { return try JSONSerialization.jsonObject(with: JSONEncoder().encode(value), options: [.fragmentsAllowed]) }
        catch { preconditionFailure("Cannot encode native DTO: \(error)") }
    }

    static func data(_ request: [String: Any]) throws -> Data {
        let data = try JSONSerialization.data(withJSONObject: request, options: [.fragmentsAllowed])
        let result = String(decoding: data, as: UTF8.self).withCString { mikomai_native_query($0) }
        defer { mikomai_result_free(result) }
        guard let message = result.message else { throw NativeStoreError.failure("Native policy returned no result") }
        let text = String(cString: message)
        guard result.status == 0 else { throw NativeStoreError.failure(text) }
        return Data(text.utf8)
    }

    static func call<T: Decodable>(_ request: [String: Any], as: T.Type = T.self) -> T {
        do { return try JSONDecoder().decode(T.self, from: data(request)) }
        catch { preconditionFailure("Native policy failed: \(error)") }
    }
}

public enum NativePersistence {
    public static func load<T: Decodable>(_ collection: String, as: T.Type = T.self) throws -> T? {
        try JSONDecoder().decode(T?.self, from: RustPolicy.data(["op": "store_load", "collection": collection]))
    }
    public static func save<T: Encodable>(_ value: T, collection: String) throws {
        let data = try JSONEncoder().encode(value)
        let object = try JSONSerialization.jsonObject(with: data, options: [.fragmentsAllowed])
        _ = try RustPolicy.data(["op": "store_save", "collection": collection, "value": object])
    }
}

public enum LegacyDataNotice {
    public static var existingDirectories: [String] { RustPolicy.call(["op": "legacy_locations"]) }
}
