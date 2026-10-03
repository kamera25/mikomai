import AppKit

/// Present without a nested modal run loop. Attach to the current window when
/// available; startup or windowless callers use an asynchronous standalone panel.
@MainActor
enum FilePanelPresenter {
    static func present(
        _ panel: NSSavePanel,
        completion: @escaping @MainActor (NSApplication.ModalResponse) -> Void
    ) {
        if let window = NSApp.keyWindow ?? NSApp.mainWindow {
            guard window.attachedSheet == nil else { return }
            panel.beginSheetModal(for: window, completionHandler: completion)
        } else {
            panel.begin(completionHandler: completion)
        }
    }
}
