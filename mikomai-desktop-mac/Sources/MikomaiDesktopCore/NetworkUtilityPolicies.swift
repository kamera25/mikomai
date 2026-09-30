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
