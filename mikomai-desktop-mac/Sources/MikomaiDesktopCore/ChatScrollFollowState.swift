/// Tracks whether new chat output should continue to keep the viewport at the end.
public struct ChatScrollFollowState: Equatable {
    public private(set) var followsOutput = true
    public private(set) var lastContentTop: Double?
    private var pendingUpwardMovement = false

    public init() {}

    /// A rising content top signals movement toward older content. The bottom
    /// preference can arrive one update later, so confirm that movement once
    /// the viewport reports that it left the end.
    public mutating func observe(contentTop: Double, isAtBottom: Bool) {
        if let previous = lastContentTop {
            if contentTop > previous + 1 {
                if isAtBottom { pendingUpwardMovement = true }
                else { followsOutput = false }
            } else if contentTop < previous - 1, isAtBottom {
                pendingUpwardMovement = false
            }
        }
        if pendingUpwardMovement, !isAtBottom {
            followsOutput = false
            pendingUpwardMovement = false
        }
        lastContentTop = contentTop
    }

    public mutating func resume() {
        followsOutput = true
        pendingUpwardMovement = false
    }

    public mutating func updateViewport(isAtBottom: Bool) {
        if pendingUpwardMovement, !isAtBottom {
            followsOutput = false
            pendingUpwardMovement = false
        }
    }

    public mutating func resetForSessionChange() {
        followsOutput = true
        lastContentTop = nil
        pendingUpwardMovement = false
    }
}
