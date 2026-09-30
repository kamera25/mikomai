import Foundation

/// Reads the same registry as Tauri's host suggestions without importing
/// credentials or modifying either app's connection inventory.
public enum HostCompletionSource {
    private struct Entry: Decodable {
        let hostname: String
        let ip: String?
    }

    public static func decode(_ data: Data) throws -> [HostSuggestion] {
        let entries = try JSONDecoder().decode([Entry].self, from: data)
        return entries.compactMap { entry in
            let name = entry.hostname.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !name.isEmpty else { return nil }
            let ip = entry.ip?.trimmingCharacters(in: .whitespacesAndNewlines)
            return HostSuggestion(hostname: name, ip: ip?.isEmpty == false ? ip! : "Console")
        }
    }

    public static func read(from url: URL) -> [HostSuggestion] {
        guard let size = try? url.resourceValues(forKeys: [.fileSizeKey]).fileSize,
              size <= 2 * 1024 * 1024,
              let data = try? Data(contentsOf: url), let hosts = try? decode(data) else { return [] }
        return hosts
    }

    public static func merge(registry: [HostSuggestion], native: [HostSuggestion]) -> [HostSuggestion] {
        var seen = Set<String>()
        // Local edits take precedence, while imported registry hosts remain
        // available even when the native inventory has not been imported.
        return (native + registry).filter { seen.insert($0.hostname).inserted }
    }
}
