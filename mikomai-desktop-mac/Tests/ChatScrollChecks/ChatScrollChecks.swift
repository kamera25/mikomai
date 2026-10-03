import MikomaiDesktopCore

@main struct ChatScrollChecks {
    @MainActor static func main() {
        setenv("MIKOMAI_SETTINGS_PATH", "/private/tmp/mikomai-full-check/settings.json", 1)
        let app = NSApplication.shared
        app.accessibilitySetValue(true, forAttribute: NSAccessibility.Attribute(rawValue: "AXEnhancedUserInterface"))
        let domain = "Mikomai.ChatScrollChecks.\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: domain)!
        defer { defaults.removePersistentDomain(forName: domain) }
        let model = DesktopModel(defaults: defaults)
        let session = ChatSession(title: "スクロール表示確認", messages: (0..<30).map {
            ChatMessage(role: .assistant, text: "メッセージ \($0)\n" + String(repeating: "スクロール検証用の本文です。\n", count: 8))
        })
        model.sessions = [session]
        model.activeSessionID = session.id
        let host = NSHostingView(rootView: DesktopWindow(model: model))
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 1200, height: 800),
                              styleMask: [.titled], backing: .buffered, defer: false)
        window.contentView = host
        window.orderFront(nil)
        func pump() {
            RunLoop.main.run(until: Date().addingTimeInterval(0.5))
            host.layoutSubtreeIfNeeded()
        }
        func descendants(_ view: NSView) -> [NSView] {
            [view] + view.subviews.flatMap(descendants)
        }
        func attribute(_ object: NSObject, _ name: String) -> Any? {
            let selector = NSSelectorFromString(name)
            guard object.responds(to: selector) else { return nil }
            return object.perform(selector)?.takeUnretainedValue()
        }
        func button(_ element: Any) -> NSObject? {
            guard let accessible = element as? NSObject else { return nil }
            if attribute(accessible, "accessibilityIdentifier") as? String == "chat-scroll-to-bottom"
                || (attribute(accessible, "accessibilityLabel") as? String)?.contains("一番下に移動") == true { return accessible }
            let children = (attribute(accessible, "accessibilityChildren") as? [Any] ?? [])
                + ((element as? NSView)?.subviews ?? [])
            return children.lazy.compactMap(button).first
        }
        pump()
        guard let scroll = descendants(host).compactMap({ $0 as? NSScrollView })
            .first(where: { descendants($0).contains(where: { $0 is ChatScrollObservationView }) }),
              let document = scroll.documentView else { fatalError("chat scroll view not found") }
        func scrollTo(_ fraction: CGFloat) {
            // Lazy row estimates settle as the destination's messages mount.
            for _ in 0..<4 {
                let maxOffset = max(0, document.bounds.height - scroll.contentView.bounds.height)
                scroll.contentView.scroll(to: NSPoint(x: 0, y: maxOffset * fraction))
                scroll.reflectScrolledClipView(scroll.contentView)
                pump()
            }
        }
        scrollTo(1)
        precondition(button(host) == nil, "button must be hidden at bottom")
        scrollTo(0.5)
        let bitmap = host.bitmapImageRepForCachingDisplay(in: host.bounds)!
        host.cacheDisplay(in: host.bounds, to: bitmap)
        try! bitmap.representation(using: .png, properties: [:])!.write(
            to: URL(fileURLWithPath: "/private/tmp/mikomai-full-check/scroll-button.png"))
        precondition(button(host) != nil, "button must appear midway through long chat")
        scrollTo(0)
        precondition(button(host) != nil, "button must remain visible when bottom rows are unmounted")
        let target = button(host)!
        let press = NSSelectorFromString("accessibilityPerformPress")
        precondition(target.responds(to: press), "button must expose a press action")
        typealias Press = @convention(c) (AnyObject, Selector) -> Bool
        let performPress = unsafeBitCast(target.method(for: press), to: Press.self)
        precondition(performPress(target, press), "button must accept a click")
        pump()
        precondition(button(host) == nil, "button must disappear after returning to bottom")
        precondition(document.bounds.maxY - scroll.documentVisibleRect.maxY <= 1,
                     "click must reach the actual bottom, including padding")
        window.setContentSize(NSSize(width: 1200, height: 500))
        pump()
        let remaining = document.bounds.maxY - scroll.documentVisibleRect.maxY
        precondition((button(host) != nil) == (remaining > 1), "resize must update button visibility")
        print("PASS: production chat button appears at middle/top, returns to bottom, and follows resize")
        _ = app
    }
}
