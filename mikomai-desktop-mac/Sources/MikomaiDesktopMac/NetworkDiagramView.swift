import SwiftUI
import AppKit
import WebKit
import UniformTypeIdentifiers
import MikomaiDesktopCore

struct NetworkDiagramView: View {
    let title: String
    let image: ChatDiagramImage
    @Environment(\.expandNetworkDiagram) private var expandDiagram
    @State private var saveError: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(title.isEmpty ? "NW図" : title).font(.headline)
            DiagramSVGView(source: image.source)
                .aspectRatio(image.aspectRatio, contentMode: .fit)
                .frame(maxWidth: .infinity, maxHeight: 600)
                .accessibilityLabel(title.isEmpty ? "ネットワーク構成図" : title)
                .overlay(alignment: .bottomTrailing) {
                    DiagramControls(expanded: false, onExpand: {
                        expandDiagram(image)
                    }, onSave: {
                        saveDiagram(image) { saveError = $0 }
                    })
                    .padding(12)
                }
            if let saveError { Text(saveError).font(.caption).foregroundStyle(.red) }
        }
        .padding(10)
        .background(Color(nsColor: .controlBackgroundColor), in: RoundedRectangle(cornerRadius: 6))
    }
}

/// A compact PiP-style control surface, floating over the image itself.
private struct DiagramControls: View {
    let expanded: Bool
    let onExpand: () -> Void
    let onSave: () -> Void

    var body: some View {
        HStack(spacing: 4) {
            action(expanded ? "arrow.down.right.and.arrow.up.left" : "arrow.up.left.and.arrow.down.right",
                   label: expanded ? "チャットへ戻る" : "チャット画面いっぱいに拡大", action: onExpand)
            action("square.and.arrow.down", label: "SVGを保存", action: onSave)
        }
        .padding(6)
        .modifier(DiagramGlassSurface())
    }

    private func action(_ symbol: String, label: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: symbol)
                .font(.system(size: 17, weight: .medium))
                .frame(width: 36, height: 36)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .help(label)
        .accessibilityLabel(label)
    }
}

private struct DiagramGlassSurface: ViewModifier {
    @ViewBuilder func body(content: Content) -> some View {
        if #available(macOS 26.0, *) {
            content.glassEffect(.regular.interactive(), in: Capsule())
        } else {
            content.background(.ultraThinMaterial, in: Capsule())
                .overlay { Capsule().strokeBorder(.white.opacity(0.3), lineWidth: 0.5) }
        }
    }
}

private struct ExpandNetworkDiagramKey: EnvironmentKey {
    static let defaultValue: @MainActor @Sendable (ChatDiagramImage) -> Void = { _ in }
}

extension EnvironmentValues {
    var expandNetworkDiagram: @MainActor @Sendable (ChatDiagramImage) -> Void {
        get { self[ExpandNetworkDiagramKey.self] }
        set { self[ExpandNetworkDiagramKey.self] = newValue }
    }
}

@MainActor
final class NetworkDiagramPresentation: ObservableObject {
    @Published private(set) var image: ChatDiagramImage?
    func show(_ image: ChatDiagramImage) { self.image = image }
    func close() { image = nil }
}

struct ChatDiagramExpansion: ViewModifier {
    @ObservedObject var presentation: NetworkDiagramPresentation

    func body(content: Content) -> some View {
        content
            .environment(\.expandNetworkDiagram, { presentation.show($0) })
            .allowsHitTesting(presentation.image == nil)
            .accessibilityHidden(presentation.image != nil)
            .overlay {
                if let image = presentation.image {
                    ExpandedNetworkDiagram(image: image, onClose: presentation.close)
                }
            }
    }
}

private struct ExpandedNetworkDiagram: View {
    let image: ChatDiagramImage
    let onClose: () -> Void
    @State private var saveError: String?

    var body: some View {
        GeometryReader { geometry in
            ZStack(alignment: .bottomTrailing) {
                Color.white
                if let rendered = NSImage(data: image.data) {
                    Image(nsImage: rendered)
                        .resizable()
                        .scaledToFit()
                        .frame(width: geometry.size.width, height: geometry.size.height)
                        .accessibilityLabel("ネットワーク構成図")
                } else {
                    DiagramSVGView(source: image.source)
                        .frame(width: geometry.size.width, height: geometry.size.height)
                }
                VStack(alignment: .trailing, spacing: 8) {
                    if let saveError {
                        Text(saveError).foregroundStyle(.red).padding(10)
                            .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 8))
                    }
                    DiagramControls(expanded: true, onExpand: onClose, onSave: {
                        saveDiagram(image) { saveError = $0 }
                    })
                }.padding(24)
            }
        }
        .onExitCommand(perform: onClose)
    }
}

@MainActor
private func saveDiagram(_ image: ChatDiagramImage, onError: @escaping @MainActor (String?) -> Void) {
    let panel = NSSavePanel()
    panel.allowedContentTypes = [UTType(filenameExtension: "svg") ?? .xml]
    panel.nameFieldStringValue = "network.svg"
    FilePanelPresenter.present(panel) { response in
        guard response == .OK, let url = panel.url else { return }
        do { try image.data.write(to: url, options: .atomic); onError(nil) }
        catch { onError("SVGを保存できません: \(error.localizedDescription)") }
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
        <style>html,body{margin:0;width:100%;height:100%;overflow:auto;background:white}img{display:block;width:100%;height:100%;object-fit:contain}</style>
        </head><body><img alt="Network diagram" src="\(source)"></body></html>
        """, baseURL: nil)
    }
    func makeCoordinator() -> Coordinator { Coordinator() }
    final class Coordinator { var source: String? }
}
