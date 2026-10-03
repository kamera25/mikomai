import AppKit
import SwiftUI

/// Reads the actual clip view, independent of which lazy message rows are mounted.
struct ChatScrollObserver: NSViewRepresentable {
    let onChange: (Double, Bool) -> Void

    func makeNSView(context: Context) -> ChatScrollObservationView {
        let view = ChatScrollObservationView()
        view.onChange = onChange
        return view
    }

    func updateNSView(_ view: ChatScrollObservationView, context: Context) {
        view.onChange = onChange
        view.scheduleObservation()
    }

    static func dismantleNSView(_ view: ChatScrollObservationView, coordinator: ()) {
        view.stopObserving()
    }
}

final class ChatScrollObservationView: NSView {
    var onChange: ((Double, Bool) -> Void)?
    private weak var observedScrollView: NSScrollView?
    private var observers: [NSObjectProtocol] = []
    private var updatePending = false
    private var lastTop: Double?
    private var lastAtBottom: Bool?

    override func hitTest(_ point: NSPoint) -> NSView? { nil }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        scheduleObservation()
    }

    override func viewDidMoveToSuperview() {
        super.viewDidMoveToSuperview()
        scheduleObservation()
    }

    func stopObserving() {
        observers.forEach { NotificationCenter.default.removeObserver($0) }
        observers.removeAll()
        observedScrollView = nil
    }

    func scheduleObservation() {
        guard !updatePending else { return }
        updatePending = true
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            self.updatePending = false
            self.observe()
        }
    }

    private func observe() {
        guard let scroll = enclosingScrollView, let document = scroll.documentView else { return }
        if observedScrollView !== scroll {
            stopObserving()
            observedScrollView = scroll
            lastTop = nil
            lastAtBottom = nil
            scroll.contentView.postsBoundsChangedNotifications = true
            document.postsFrameChangedNotifications = true
            for (name, object) in [
                (NSView.boundsDidChangeNotification, scroll.contentView),
                (NSView.frameDidChangeNotification, document)
            ] {
                observers.append(NotificationCenter.default.addObserver(
                    forName: name, object: object, queue: .main
                ) { [weak self] _ in
                    MainActor.assumeIsolated { self?.scheduleObservation() }
                })
            }
        }
        let visible = scroll.documentVisibleRect
        let bounds = document.bounds
        let top = Double(document.isFlipped ? bounds.minY - visible.minY : visible.maxY - bounds.maxY)
        let remaining = document.isFlipped ? bounds.maxY - visible.maxY : visible.minY - bounds.minY
        let atBottom = remaining <= 1
        guard top != lastTop || atBottom != lastAtBottom else { return }
        lastTop = top
        lastAtBottom = atBottom
        onChange?(top, atBottom)
    }
}
