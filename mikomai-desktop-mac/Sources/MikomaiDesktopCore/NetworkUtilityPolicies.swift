import Foundation
import Darwin

public enum IPAddressPolicy {
    public static func isGlobalIP(_ value: String) -> Bool {
        let ipv4Parts = value.split(separator: ".", omittingEmptySubsequences: false)
        if ipv4Parts.count == 4,
           ipv4Parts.allSatisfy({ !$0.isEmpty && $0.allSatisfy(\.isNumber) }) {
            let octets = ipv4Parts.compactMap { UInt16($0) }
            guard octets.count == 4, octets.allSatisfy({ $0 <= 255 }) else { return false }
            let first = octets[0]
            let second = octets[1]
            if first == 127 || first == 10 || (first == 172 && (16...31).contains(second))
                || (first == 192 && second == 168) || (first == 169 && second == 254)
                || first == 0 || (224...239).contains(first) || first >= 240 {
                return false
            }
            return true
        }

        var address = in6_addr()
        guard value.withCString({ inet_pton(AF_INET6, $0, &address) == 1 }) else { return false }
        let normalized = value.lowercased()
        if normalized == "::1" || normalized == "0:0:0:0:0:0:0:1" { return false }
        if normalized.hasPrefix("fe8") || normalized.hasPrefix("fe9")
            || normalized.hasPrefix("fea") || normalized.hasPrefix("feb")
            || normalized.hasPrefix("fc") || normalized.hasPrefix("fd")
            || normalized.hasPrefix("ff") {
            return false
        }
        return true
    }
}

public struct PingCommand: Equatable {
    public var host: String
    public var size: Int?
    public var count: Int?
    public var df: Bool?

    public init(host: String, size: Int? = nil, count: Int? = nil, df: Bool? = nil) {
        self.host = host
        self.size = size
        self.count = count
        self.df = df
    }

    public var processArguments: [String]? {
        guard !host.isEmpty, !host.hasPrefix("-"),
              host.range(of: "^[A-Za-z0-9._:%-]+$", options: .regularExpression) != nil else { return nil }
        if let size, !(1...65_500).contains(size) { return nil }
        var result = ["-c", "\(min(max(count ?? 4, 1), 10))"]
        if let size { result += ["-s", "\(size)"] }
        if df == true { result.append("-D") }
        result.append(host)
        return result
    }
}

/// Preserves the former `get_state(resource: cpu)` read-only contract: a
/// vendor-specific show command is run, then only a finite 0...100 usage value
/// is returned to a Watch condition.
public enum CPUUsagePolicy {
    public static func command(for deviceType: String) -> String {
        let type = deviceType.lowercased()
        if type.contains("juniper") { return "show system processes extensive | match CPU" }
        if type.contains("arista") { return "show processes top once" }
        if type.contains("yamaha") { return "show status cpu" }
        if type.contains("furukawa") || type.contains("fitel") { return "show cpu" }
        return "show processes cpu"
    }

    public static func parse(_ output: String) -> Double? {
        let patterns = [
            #"(?i)cpu\s+utilization[^\n:]*:\s*(\d+(?:\.\d+)?)\s*%"#,
            #"(?i)cpu[^\n]*?\b(\d+(?:\.\d+)?)\s*(?:%|percent)\b"#
        ]
        for pattern in patterns {
            guard let regex = try? NSRegularExpression(pattern: pattern),
                  let match = regex.firstMatch(in: output, range: NSRange(output.startIndex..., in: output)),
                  let range = Range(match.range(at: 1), in: output),
                  let value = Double(output[range]), value.isFinite, (0...100).contains(value) else { continue }
            return value
        }
        return nil
    }
}

public enum PingCommandParser {
    public static func parse(_ input: String) -> PingCommand? {
        let lowered = input.lowercased()
        let basePatterns = [
            #"(?:ping|ピン|ピング)\s+([a-zA-Z0-9.:-]+)"#,
            #"([a-zA-Z0-9.:-]+)\s*(?:に|へ)?\s*(?:ping|ピン|ピング)"#
        ]
        guard let host = basePatterns.lazy.compactMap({ firstCapture($0, in: lowered) }).first else { return nil }

        let size = firstCapture(#"(?:size|サイズ)\s*(\d+)"#, in: lowered).flatMap(Int.init)
        let count = firstCapture(#"(?:count|回数|回)\s*(\d+)"#, in: lowered).flatMap(Int.init)
            ?? firstCapture(#"(\d+)\s*回(?:実行)?"#, in: lowered).flatMap(Int.init)
        let hasDoNotFragment = ["df", "フラグメント禁止", "断片化禁止"].contains(where: lowered.contains)
        return PingCommand(host: host, size: size, count: count, df: hasDoNotFragment ? true : nil)
    }

    private static func firstCapture(_ pattern: String, in value: String) -> String? {
        guard let expression = try? NSRegularExpression(pattern: pattern),
              let match = expression.firstMatch(in: value, range: NSRange(value.startIndex..., in: value)),
              let range = Range(match.range(at: 1), in: value) else { return nil }
        return String(value[range])
    }
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
    public static func detectError(in output: String) -> NetworkDeviceError? {
        let lower = output.lowercased()
        if lower.contains("% invalid input") {
            return .invalidInput("% invalid input")
        }
        if lower.contains("% incomplete command") {
            return .incompleteCommand("% incomplete command")
        }
        if lower.contains("% ambiguous command") {
            return .ambiguousCommand("% ambiguous command")
        }
        if lower.contains("syntax error") {
            return .syntaxError("syntax error")
        }
        if lower.contains("netmiko error:") {
            return .netmikoError("netmiko error:")
        }
        if lower.contains("error: device") {
            return .deviceError("error: device")
        }
        return nil
    }

    public static func hasError(in output: String) -> Bool {
        detectError(in: output) != nil
    }
}


/// ARP commands from the former Tauri vendor templates.
public enum ARPCommandPolicy {
    public static func command(for deviceType: String) -> String {
        let type = DeviceTypeCatalog.canonicalID(for: deviceType)
        if type.contains("juniper") || type.contains("yamaha") { return "show arp" }
        return "show ip arp"
    }
}


/// Resolve saved display names before an OS network probe can attempt DNS.
public enum RegisteredDiagnosticHostPolicy {
    public static func resolve(_ requestedHost: String, connections: [SavedConnection]) throws -> String {
        let host = requestedHost.trimmingCharacters(in: .whitespacesAndNewlines)
        let matches = connections.filter {
            $0.name.trimmingCharacters(in: .whitespacesAndNewlines).caseInsensitiveCompare(host) == .orderedSame
                || $0.id.uuidString.caseInsensitiveCompare(host) == .orderedSame
                || $0.sourceID == host
        }
        guard matches.count <= 1 else { throw ResolutionError.ambiguous }
        guard let connection = matches.first else { return host }
        let address = connection.host.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !address.isEmpty else { throw ResolutionError.missingAddress }
        guard isIPAddressLiteral(address) else { throw ResolutionError.invalidAddress }
        return address
    }

    private static func isIPAddressLiteral(_ value: String) -> Bool {
        var ipv4 = in_addr()
        if value.withCString({ inet_pton(AF_INET, $0, &ipv4) }) == 1 { return true }
        var ipv6 = in6_addr()
        return value.withCString({ inet_pton(AF_INET6, $0, &ipv6) }) == 1
    }

    public enum ResolutionError: LocalizedError {
        case ambiguous, missingAddress, invalidAddress
        public var errorDescription: String? {
            switch self {
            case .ambiguous: return "同じ名前の登録機器が複数あります。対象のIPアドレスを指定してください。"
            case .missingAddress: return "登録機器の接続先が未設定です。IPアドレスを設定してください。"
            case .invalidAddress: return "登録機器のホスト欄に有効なIPアドレスがありません。名前解決を避けるため、IPアドレスを登録してください。"
            }
        }
    }
}
