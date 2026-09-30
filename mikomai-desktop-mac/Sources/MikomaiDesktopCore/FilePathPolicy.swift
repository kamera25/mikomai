import Foundation

public enum FilePathPolicy {
    public static func defaultFilename(_ path: String) -> String {
        let normalized = path.replacingOccurrences(of: "\\", with: "/")
        let filename = normalized.components(separatedBy: "/").last ?? ""
        return filename.isEmpty ? "downloaded_file.txt" : filename
    }
}
