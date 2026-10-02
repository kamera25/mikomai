import Foundation
import CoreGraphics

/// Shared sizing rules for the resizable side panes and responsive window tiling.
public enum PaneResizePolicy {
    public static let minimumWidth = 180.0
    public static let compactWidthThreshold = 960.0

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

    public static func isCompactWidth(_ width: Double) -> Bool {
        width <= compactWidthThreshold
    }

    public static func isWindowTiledToSide(
        windowFrame: CGRect,
        screenVisibleFrame: CGRect,
        tolerance: CGFloat = 32
    ) -> Bool {
        guard screenVisibleFrame.width > 0, screenVisibleFrame.height > 0 else { return false }

        let isNearFullHeight = windowFrame.height >= (screenVisibleFrame.height - 60)
        let isRoughlyHalfWidth = windowFrame.width >= (screenVisibleFrame.width * 0.35) &&
                                 windowFrame.width <= (screenVisibleFrame.width * 0.65)

        let isAlignedToLeft = abs(windowFrame.minX - screenVisibleFrame.minX) <= tolerance
        let isAlignedToRight = abs(windowFrame.maxX - screenVisibleFrame.maxX) <= tolerance

        return (isNearFullHeight || windowFrame.height >= screenVisibleFrame.height * 0.45) &&
               isRoughlyHalfWidth &&
               (isAlignedToLeft || isAlignedToRight)
    }

    public static func shouldCollapsePanesForTiling(
        containerWidth: Double,
        windowFrame: CGRect? = nil,
        screenVisibleFrame: CGRect? = nil
    ) -> Bool {
        if isCompactWidth(containerWidth) {
            return true
        }
        if let windowFrame = windowFrame, let screenVisibleFrame = screenVisibleFrame {
            if isWindowTiledToSide(windowFrame: windowFrame, screenVisibleFrame: screenVisibleFrame) {
                return true
            }
        }
        return false
    }
}

