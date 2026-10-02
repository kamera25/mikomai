import SwiftUI
import MikomaiDesktopCore

struct SessionRow: View {
    let session: ChatSession
    let isSelected: Bool
    let onSelect: () -> Void
    let onRename: (String) -> Void
    let onDelete: () -> Void
    @State private var isRenaming = false
    @State private var isHovered = false
    @State private var title = ""

    var body: some View {
        Group {
            if isRenaming {
                TextField("会話名", text: $title, onCommit: { onRename(title); isRenaming = false })
                    .textFieldStyle(.plain).font(.system(size: 14))
                    .padding(.horizontal, 8).padding(.vertical, 8)
            } else {
                HistorySelectionRow(isSelected: isSelected, action: onSelect) {
                    HoverScrollTitle(title: session.title, isHovered: isHovered)
                }
            }
        }
        .frame(maxWidth: .infinity)
        .background(isRenaming && isSelected ? Color(nsColor: .selectedContentBackgroundColor).opacity(0.16) : .clear, in: RoundedRectangle(cornerRadius: 5))
        .contentShape(Rectangle())
        .onHover { hovering in
            isHovered = hovering
        }
        .contextMenu {
            Button("名前を変更") { title = session.title; isRenaming = true }
            Button("削除", role: .destructive, action: onDelete)
        }
    }
}

struct HoverScrollTitle: View {
    let title: String
    var isHovered: Bool = false
    var font: Font = .system(size: 14)
    var fontSize: CGFloat = 14

    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var isSelfHovered = false
    @State private var measuredTextWidth: CGFloat = 0
    @State private var isScrolling = false
    @State private var scrollOffset: CGFloat = 0
    @State private var scrollTask: Task<Void, Never>?

    private var effectiveHovered: Bool {
        isHovered || isSelfHovered
    }

    private var effectiveTextWidth: CGFloat {
        if measuredTextWidth > 0 {
            return measuredTextWidth
        }
        return CGFloat(HoverScrollPolicy.textWidth(for: title, fontSize: fontSize))
    }

    var body: some View {
        GeometryReader { proxy in
            let containerWidth = proxy.size.width
            let textWidth = effectiveTextWidth
            let isTruncated = HoverScrollPolicy.isTruncated(textWidth: Double(textWidth), containerWidth: Double(containerWidth))

            ZStack(alignment: .leading) {
                // Background measurement to determine pixel-accurate SwiftUI text width
                Text(title)
                    .font(font)
                    .lineLimit(1)
                    .fixedSize(horizontal: true, vertical: false)
                    .background(
                        GeometryReader { textGeo in
                            Color.clear.preference(key: TitleWidthPreferenceKey.self, value: textGeo.size.width)
                        }
                    )
                    .hidden()

                if isTruncated && isScrolling && !reduceMotion {
                    Text(title)
                        .font(font)
                        .lineLimit(1)
                        .fixedSize(horizontal: true, vertical: false)
                        .offset(x: scrollOffset)
                        .frame(width: containerWidth, alignment: .leading)
                        .clipped()
                } else {
                    Text(title)
                        .font(font)
                        .lineLimit(1)
                        .truncationMode(.tail)
                        .frame(width: containerWidth, alignment: .leading)
                }
            }
            .onPreferenceChange(TitleWidthPreferenceKey.self) { newWidth in
                if newWidth > 0 && abs(newWidth - measuredTextWidth) > 0.5 {
                    measuredTextWidth = newWidth
                }
            }
            .onAppear {
                syncScroll(hovering: effectiveHovered, containerWidth: containerWidth, textWidth: textWidth)
            }
            .onChange(of: effectiveHovered) { hovering in
                syncScroll(hovering: hovering, containerWidth: containerWidth, textWidth: textWidth)
            }
            .onChange(of: title) { _ in
                measuredTextWidth = 0
                syncScroll(hovering: effectiveHovered, containerWidth: containerWidth, textWidth: effectiveTextWidth)
            }
            .onChange(of: containerWidth) { newContainerWidth in
                syncScroll(hovering: effectiveHovered, containerWidth: newContainerWidth, textWidth: textWidth)
            }
        }
        .frame(height: 20)
        .onHover { isSelfHovered = $0 }
        .onDisappear {
            scrollTask?.cancel()
            scrollTask = nil
            isScrolling = false
            scrollOffset = 0
        }
    }

    private func syncScroll(hovering: Bool, containerWidth: CGFloat, textWidth: CGFloat) {
        scrollTask?.cancel()
        scrollTask = nil

        guard hovering, !reduceMotion, HoverScrollPolicy.isTruncated(textWidth: Double(textWidth), containerWidth: Double(containerWidth)) else {
            withAnimation(.easeOut(duration: 0.2)) {
                scrollOffset = 0
            }
            isScrolling = false
            return
        }

        let overflow = textWidth - containerWidth
        let duration = HoverScrollPolicy.scrollDuration(overflow: Double(overflow))

        scrollTask = Task { @MainActor in
            try? await Task.sleep(nanoseconds: UInt64(HoverScrollPolicy.hoverInitialDelay * 1_000_000_000))
            guard !Task.isCancelled else { return }

            isScrolling = true
            while !Task.isCancelled {
                withAnimation(.easeInOut(duration: duration)) {
                    scrollOffset = -overflow
                }
                try? await Task.sleep(nanoseconds: UInt64((duration + HoverScrollPolicy.hoverEndPause) * 1_000_000_000))
                guard !Task.isCancelled else { return }

                withAnimation(.easeInOut(duration: duration)) {
                    scrollOffset = 0
                }
                try? await Task.sleep(nanoseconds: UInt64((duration + HoverScrollPolicy.hoverEndPause) * 1_000_000_000))
                guard !Task.isCancelled else { return }
            }
        }
    }
}

private struct TitleWidthPreferenceKey: PreferenceKey {
    static let defaultValue: CGFloat = 0
    static func reduce(value: inout CGFloat, nextValue: () -> CGFloat) {
        let next = nextValue()
        if next > 0 { value = next }
    }
}
