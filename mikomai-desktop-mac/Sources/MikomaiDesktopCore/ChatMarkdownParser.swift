import Foundation

public struct ChatMarkdownBlock: Equatable {
    public enum Kind: Equatable {
        case heading(Int, String)
        case paragraph(String)
        case code(String, String)
        case bullet(String)
        case quote(String)
        case image(String, String)
        case imageFile(URL)
        case separator
    }

    public let kind: Kind

    public init(kind: Kind) { self.kind = kind }
}

public enum ChatMarkdownParser {
    public static func parse(_ source: String) -> [ChatMarkdownBlock] {
        let lines = source.components(separatedBy: .newlines)
        var result: [ChatMarkdownBlock] = []
        var paragraph: [String] = []
        var index = 0

        func flushParagraph() {
            guard !paragraph.isEmpty else { return }
            // Chat answers use single newlines for intentional line breaks.
            result.append(ChatMarkdownBlock(kind: .paragraph(paragraph.joined(separator: "\n"))))
            paragraph.removeAll(keepingCapacity: true)
        }

        while index < lines.count {
            let line = lines[index]
            let trimmed = line.trimmingCharacters(in: .whitespaces)
            if trimmed.isEmpty { flushParagraph(); index += 1; continue }
            let savedImagePrefix = "SVGを保存しました: "
            if trimmed.hasPrefix(savedImagePrefix) {
                let path = String(trimmed.dropFirst(savedImagePrefix.count))
                let url = URL(fileURLWithPath: path)
                if path.hasPrefix("/"), url.pathExtension.lowercased() == "svg" {
                    flushParagraph()
                    result.append(ChatMarkdownBlock(kind: .imageFile(url)))
                    index += 1
                    continue
                }
            }
            if trimmed.hasPrefix("!["), let divider = trimmed.range(of: "]("), trimmed.hasSuffix(")") {
                let title = String(trimmed[trimmed.index(trimmed.startIndex, offsetBy: 2)..<divider.lowerBound])
                let source = String(trimmed[divider.upperBound..<trimmed.index(before: trimmed.endIndex)])
                if ChatDiagramImage(source: source) != nil {
                    flushParagraph()
                    result.append(ChatMarkdownBlock(kind: .image(title, source)))
                    index += 1
                    continue
                }
            }
            if trimmed.hasPrefix("```") {
                flushParagraph()
                let language = String(trimmed.dropFirst(3)).trimmingCharacters(in: .whitespaces)
                index += 1
                var code: [String] = []
                while index < lines.count && !lines[index].trimmingCharacters(in: .whitespaces).hasPrefix("```") {
                    code.append(lines[index]); index += 1
                }
                if index < lines.count { index += 1 }
                result.append(ChatMarkdownBlock(kind: .code(language, code.joined(separator: "\n"))))
                continue
            }
            if ["---", "***", "___"].contains(trimmed) {
                flushParagraph(); result.append(ChatMarkdownBlock(kind: .separator)); index += 1; continue
            }
            let hashCount = trimmed.prefix(while: { $0 == "#" }).count
            if (1...6).contains(hashCount), trimmed.dropFirst(hashCount).first == " " {
                flushParagraph()
                result.append(ChatMarkdownBlock(kind: .heading(hashCount, String(trimmed.dropFirst(hashCount)).trimmingCharacters(in: .whitespaces))))
                index += 1
                continue
            }
            if trimmed.hasPrefix("> ") {
                flushParagraph(); result.append(ChatMarkdownBlock(kind: .quote(String(trimmed.dropFirst(2))))); index += 1; continue
            }
            if ["- ", "* ", "+ "].contains(where: { trimmed.hasPrefix($0) }) {
                flushParagraph(); result.append(ChatMarkdownBlock(kind: .bullet(String(trimmed.dropFirst(2))))); index += 1; continue
            }
            paragraph.append(trimmed)
            index += 1
        }
        flushParagraph()
        return result
    }
}
