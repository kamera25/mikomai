import WebKit
import MikomaiBindings
import MikomaiDesktopCore

@main struct NetworkDiagramChecks {
    @MainActor static func main() {
        let root = ProcessInfo.processInfo.environment["MIKOMAI_DIAGRAM_CHECK_DIR"] ?? "/private/tmp/mikomai-diagram-check"
        try! FileManager.default.createDirectory(atPath: root, withIntermediateDirectories: true)
        let documents = "\(root)/documents"
        try! FileManager.default.createDirectory(atPath: documents, withIntermediateDirectories: true)
        setenv("MIKOMAI_ARTIFACTS_DIR", root, 1)
        setenv("MIKOMAI_DATA_DIR", "\(root)/data", 1)
        setenv("MIKOMAI_GRAPH_DB_PATH", "\(root)/graph", 1)
        setenv("MIKOMAI_E5_CACHE_DIR", "\(root)/e5", 1)
        let schema = "nwdiag {\n network lan {\n address = \"192.168.1.0/24\";\n router01 [address = \"192.168.1.1\"];\n switch01 [address = \"192.168.1.2\"];\n }\n}"
        var records: [String] = []
        var answer = DesktopModel.askRustStreaming("次のNW図を表示して\n```nwdiag\n\(schema)\n```", history: "", documents: documents, knowledge: documents, attachments: "", connections: [], onOperationPlan: { _ in fatalError("drawing must not request device changes") }, onDebug: { records.append($0) }, onToolResult: { _ in }, onChunk: { _, _ in })
        precondition(!answer.hasPrefix("エラー:"), answer)
        if let modelPath = ProcessInfo.processInfo.environment["MIKOMAI_DIAGRAM_CHECK_MODEL"] {
            let loaded = DesktopModel.callRust { modelPath.withCString { mikomai_model_load($0) } }
            precondition(!loaded.hasPrefix("エラー:"), loaded)
            let request = "LAN 192.168.1.0/24にrouter01とswitch01を接続したNW図を作成して"
            let generated = DesktopModel.askRustStreaming(request, history: "", documents: documents, knowledge: documents, attachments: "", connections: [], onOperationPlan: { _ in fatalError("drawing must not request changes") }, onDebug: { records.append($0) }, onToolResult: { _ in }, onChunk: { _, _ in })
            try! records.joined(separator: "\n").write(toFile: "\(root)/debug.jsonl", atomically: true, encoding: .utf8)
            guard let block = ChatMarkdownParser.parse(generated).first(where: { if case .image = $0.kind { return true }; return false }), case let .image(_, source) = block.kind,
                  let image = ChatDiagramImage(source: source), let svg = String(data: image.data, encoding: .utf8) else { fatalError("Natural-language Plotter did not render: \(generated)") }
            precondition(svg.contains("router01") && svg.contains("switch01") && svg.contains("192.168.1.0/24"))
            precondition(!svg.contains("192.168.1.1") && !svg.contains("192.168.1.2"), "unspecified node addresses must not be invented")
            precondition(!generated.contains("__MIKOMAI_CHOICE__"), "complete topology must not request clarification")
            answer = generated
            print("PASS: configured LLM natural-language request → Plotter JSON → rendered SVG")
        }
        let blocks = ChatMarkdownParser.parse(answer)
        guard let block = blocks.first(where: { if case .image = $0.kind { return true }; return false }), case let .image(_, source) = block.kind,
              let image = ChatDiagramImage(source: source) else { fatalError("missing rendered SVG: \(answer)") }
        precondition(String(data: image.data, encoding: .utf8)!.contains("router01"))
        precondition(records.contains(where: { $0.contains("tool_response") && $0.contains("self_network_nwdiag") }))
        try! records.joined(separator: "\n").write(toFile: "\(root)/debug.jsonl", atomically: true, encoding: .utf8)
        let restored = try! JSONDecoder().decode(String.self, from: JSONEncoder().encode(answer))
        precondition(ChatMarkdownParser.parse(restored) == blocks)

        _ = NSApplication.shared
        let host = NSHostingView(rootView: MarkdownMessage(text: restored))
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 850, height: 600), styleMask: [.titled], backing: .buffered, defer: false)
        window.contentView = host
        window.orderFrontRegardless()
        func webView(_ view: NSView) -> WKWebView? {
            if let web = view as? WKWebView { return web }
            return view.subviews.lazy.compactMap { webView($0) }.first
        }
        let deadline = Date().addingTimeInterval(15)
        var web: WKWebView?
        repeat {
            RunLoop.main.run(until: Date().addingTimeInterval(0.1))
            host.layoutSubtreeIfNeeded()
            web = webView(host)
        } while (web == nil || web!.isLoading) && Date() < deadline
        guard let web, !web.isLoading else { fatalError("SVG web view failed to load") }
        precondition(web.bounds.width > 100 && web.bounds.height > 100, "diagram must have visible bounds")
        var snapshot: NSImage?
        web.takeSnapshot(with: nil) { snapshot = $0; if let error = $1 { fatalError(error.localizedDescription) } }
        while snapshot == nil && Date() < deadline { RunLoop.main.run(until: Date().addingTimeInterval(0.1)) }
        guard let snapshot, let tiff = snapshot.tiffRepresentation, let bitmap = NSBitmapImageRep(data: tiff),
              let png = bitmap.representation(using: .png, properties: [:]) else { fatalError("no SVG snapshot") }
        try! png.write(to: URL(fileURLWithPath: "\(root)/diagram.png"))
        print("PASS: production FFI → Swift nwdiag renderer → final Markdown → restored chat SVG view; snapshot: \(root)/diagram.png")
        window.close()
    }
}
