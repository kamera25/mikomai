/// Shared sizing rules for the resizable side panes.
public enum PaneResizePolicy {
    public static let minimumWidth = 180.0

    public static func maximumWidth(containerWidth: Double, reservedWidth: Double, lowerBound: Double, upperBound: Double) -> Double {
        max(lowerBound, min(upperBound, containerWidth - reservedWidth))
    }

    public static func clampedWidth(_ width: Double, maximumWidth: Double) -> Double {
        min(maximumWidth, max(minimumWidth, width))
    }

    public static func shouldClose(startWidth: Double, translation: Double, isHistoryPane: Bool) -> Bool {
        let resultingWidth = isHistoryPane ? startWidth + translation : startWidth - translation
        return resultingWidth < minimumWidth
    }
}
