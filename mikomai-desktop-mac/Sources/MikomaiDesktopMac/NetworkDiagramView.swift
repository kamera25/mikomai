import SwiftUI
import WebKit
import UniformTypeIdentifiers
import MikomaiDesktopCore

struct NetworkDiagramView: View {
    let title: String
    let image: ChatDiagramImage
    @State private var expanded = false
    @State private var saveError: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text(title.isEmpty ? "NW図" : title).font(.headline)
                Spacer()
                Button("拡大") { expanded = true }
                Button("SVGを保存") { save() }
            }.buttonStyle(.borderless)
            DiagramSVGView(source: image.source)
                .aspectRatio(image.aspectRatio, contentMode: .fit)
                .frame(maxWidth: .infinity, maxHeight: 600)
                .accessibilityLabel(title.isEmpty ? "ネットワーク構成図" : title)
            if let saveError { Text(saveError).font(.caption).foregroundStyle(.red) }
        }
        .padding(10)
        .background(Color(nsColor: .controlBackgroundColor), in: RoundedRectangle(cornerRadius: 6))
        .sheet(isPresented: $expanded) {
            VStack {
                HStack { Text("NW図").font(.headline); Spacer(); Button("閉じる") { expanded = false } }
                DiagramSVGView(source: image.source)
            }.padding().frame(minWidth: 720, minHeight: 520)
        }
    }

    private func save() {
        let panel = NSSavePanel()
        panel.allowedContentTypes = [UTType(filenameExtension: "svg") ?? .xml]
        panel.nameFieldStringValue = "network.svg"
        FilePanelPresenter.present(panel) { response in
            guard response == .OK, let url = panel.url else { return }
            do { try image.data.write(to: url, options: .atomic); saveError = nil }
            catch { saveError = "SVGを保存できません: \(error.localizedDescription)" }
        }
    }
}

/// SVG is loaded as an image, keeping scripts and external resources disabled.
struct DiagramSVGView: NSViewRepresentable {
    let source: String
    func makeNSView(context: Context) -> WKWebView {
        let configuration = WKWebViewConfiguration()
        configuration.websiteDataStore = .nonPersistent()
        configuration.defaultWebpagePreferences.allowsContentJavaScript = false
        let view = WKWebView(frame: .zero, configuration: configuration)
        view.setValue(false, forKey: "drawsBackground")
        return view
    }
    func updateNSView(_ view: WKWebView, context: Context) {
        guard context.coordinator.source != source else { return }
        context.coordinator.source = source
        // source is a validated base64 data URL and cannot contain HTML delimiters.
        view.loadHTMLString("""
        <!doctype html><html><head><meta name="viewport" content="width=device-width">
        <meta http-equiv="Content-Security-Policy" content="default-src 'none'; img-src data:; style-src 'unsafe-inline'">
        <style>html,body{margin:0;width:100%;height:100%;overflow:auto;background:white}img{width:100%;height:100%;object-fit:contain}</style>
        </head><body><img alt="Network diagram" src="\(source)"></body></html>
        """, baseURL: nil)
    }
    func makeCoordinator() -> Coordinator { Coordinator() }
    final class Coordinator { var source: String? }
}
