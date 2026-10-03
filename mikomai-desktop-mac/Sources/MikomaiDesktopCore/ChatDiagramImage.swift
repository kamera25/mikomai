import Foundation

/// Portable images are embedded in saved Markdown so history needs no temporary file.
public struct ChatDiagramImage: Equatable {
    public let data: Data
    public let source: String
    public let aspectRatio: Double

    public init?(source: String) {
        let prefix = "data:image/svg+xml;base64,"
        guard source.hasPrefix(prefix), source.utf8.count <= 8 * 1024 * 1024,
              let data = Data(base64Encoded: String(source.dropFirst(prefix.count))),
              let svg = String(data: data, encoding: .utf8),
              let rootRegex = try? NSRegularExpression(pattern: "<svg\\b[^>]*>"),
              let rootMatch = rootRegex.firstMatch(in: svg, range: NSRange(svg.startIndex..., in: svg)),
              let rootRange = Range(rootMatch.range, in: svg) else { return nil }
        let root = String(svg[rootRange])
        self.data = data
        self.source = source
        // nwdiag exports width/height; viewBox-only SVGs are also accepted.
        func attribute(_ name: String) -> String? {
            guard let regex = try? NSRegularExpression(pattern: "\\b\(name)\\s*=\\s*[\"']([^\"']+)[\"']"),
                  let match = regex.firstMatch(in: root, range: NSRange(root.startIndex..., in: root)),
                  let range = Range(match.range(at: 1), in: root) else { return nil }
            return String(root[range])
        }
        func dimension(_ name: String) -> Double? {
            attribute(name).flatMap { Double($0.replacingOccurrences(of: "px", with: "")) }
        }
        let viewBox = attribute("viewBox")?.split(whereSeparator: { $0.isWhitespace || $0 == "," }).compactMap { Double($0) }
        let width = dimension("width") ?? (viewBox?.count == 4 ? viewBox?[2] : nil) ?? 800
        let height = dimension("height") ?? (viewBox?.count == 4 ? viewBox?[3] : nil) ?? 400
        self.aspectRatio = width.isFinite && height.isFinite && width > 0 && height > 0 ? width / height : 2
    }
}
