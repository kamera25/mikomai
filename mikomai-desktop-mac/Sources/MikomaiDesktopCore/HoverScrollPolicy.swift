import Foundation
#if canImport(AppKit)
import AppKit
#endif

/// Policy for scrolling truncated titles horizontally when hovered in chat history.
public enum HoverScrollPolicy {
    /// Minimum duration for the scroll animation in seconds.
    public static let defaultMinDuration: Double = 0.8
    /// Maximum duration for the scroll animation in seconds.
    public static let defaultMaxDuration: Double = 5.0
    /// Default scroll speed in points per second.
    public static let defaultPointsPerSecond: Double = 70.0
    /// Initial delay in seconds before scrolling starts on hover.
    public static let hoverInitialDelay: Double = 0.25
    /// Pause duration at ends (start and finish) in seconds.
    public static let hoverEndPause: Double = 0.6

    /// Returns whether the text overflows the container width beyond the tolerance.
    public static func isTruncated(textWidth: Double, containerWidth: Double, tolerance: Double = 0.5) -> Bool {
        containerWidth > 0 && textWidth > containerWidth + tolerance
    }

    /// Calculates the horizontal scroll offset needed to reveal the full text.
    /// Returns a negative offset or zero.
    public static func maxScrollOffset(textWidth: Double, containerWidth: Double) -> Double {
        guard isTruncated(textWidth: textWidth, containerWidth: containerWidth) else { return 0 }
        return -(textWidth - containerWidth)
    }

    /// Calculates the animation duration based on overflow distance and speed.
    public static func scrollDuration(
        overflow: Double,
        pointsPerSecond: Double = defaultPointsPerSecond,
        minDuration: Double = defaultMinDuration,
        maxDuration: Double = defaultMaxDuration
    ) -> Double {
        guard overflow > 0 else { return 0 }
        let duration = overflow / pointsPerSecond
        return min(max(duration, minDuration), maxDuration)
    }

    /// Measures the unconstrained width of a single line of text with the given system font size.
    public static func textWidth(for text: String, fontSize: CGFloat = 14) -> Double {
        #if canImport(AppKit)
        let font = NSFont.systemFont(ofSize: fontSize)
        let attributes: [NSAttributedString.Key: Any] = [.font: font]
        let size = (text as NSString).size(withAttributes: attributes)
        return Double(ceil(size.width))
        #else
        return Double(text.count) * Double(fontSize * 0.7)
        #endif
    }
}
