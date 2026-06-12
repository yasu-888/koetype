import Cocoa
import CoreText
import SwiftUI
import QuartzCore

enum OverlayEventType: String {
    case position
    case recordingStarting = "recording-starting"
    case recordingStarted = "recording-started"
    case recordingStopped = "recording-stopped"
    case transcriptionCompleted = "transcription-completed"
    case pasteCompleted = "paste-completed"
    case copyCompleted = "copy-completed"
    case transcriptionFailed = "transcription-failed"
    case recordingError = "recording-error"
    case partialTranscription = "partial-transcription"
    case audioLevel = "audio-level"
    case status
    case hide
    case show
}

struct OverlayModel: Equatable {
    var status: String
    var message: String
    var detail: String
    var audioLevel: Double

    static let idle = OverlayModel(
        status: "idle",
        message: "待機中",
        detail: "",
        audioLevel: 0.0
    )
}

struct OverlayEffects: Equatable {
    var shouldResize = false
    var shouldShow = false
    var shouldHide = false
    var shouldReposition = false
}

struct OverlayTransition: Equatable {
    var model: OverlayModel
    var effects: OverlayEffects
    var shouldCancelIdleReset = false
}

enum OverlayEventReducer {
    static func reduce(model: OverlayModel, event: OverlayEvent) -> OverlayTransition {
        var next = model
        var effects = OverlayEffects()
        var cancelIdleReset = false

        guard let kind = event.kind else {
            return OverlayTransition(model: model, effects: effects)
        }

        switch kind {
        case .position:
            effects.shouldReposition = true
        case .recordingStarting, .recordingStarted:
            next.status = "recording"
            next.message = ""
            next.detail = ""
            cancelIdleReset = true
            effects.shouldResize = true
            effects.shouldShow = true
        case .recordingStopped:
            next.status = "transcribing"
            next.message = ""
            cancelIdleReset = true
            effects.shouldResize = true
            effects.shouldShow = true
        case .transcriptionCompleted, .pasteCompleted, .copyCompleted:
            next = .idle
            cancelIdleReset = true
            effects.shouldHide = true
        case .transcriptionFailed, .recordingError:
            next.status = "error"
            next.message = event.message ?? "エラー"
            cancelIdleReset = true
            effects.shouldResize = true
            effects.shouldShow = true
        case .partialTranscription:
            if let text = event.text {
                next.detail = text
            }
            effects.shouldResize = true
            effects.shouldShow = true
        case .audioLevel:
            next.audioLevel = event.level ?? 0.0
            effects.shouldShow = true
        case .status:
            if let status = event.status {
                next.status = status
                if status == "idle" {
                    cancelIdleReset = true
                    effects.shouldHide = true
                } else {
                    effects.shouldShow = true
                }
            }
            if let message = event.message {
                next.message = message
            }
            if let text = event.text {
                next.detail = text
            }
            effects.shouldResize = true
        case .hide:
            effects.shouldHide = true
        case .show:
            effects.shouldShow = true
        }

        return OverlayTransition(
            model: next,
            effects: effects,
            shouldCancelIdleReset: cancelIdleReset
        )
    }
}

final class OverlayState: ObservableObject {
    @Published var status: String = OverlayModel.idle.status   // 初期は非表示のまま idle 扱い
    @Published var message: String = OverlayModel.idle.message
    @Published var detail: String = OverlayModel.idle.detail
    @Published var audioLevel: Double = OverlayModel.idle.audioLevel

    var model: OverlayModel {
        get {
            OverlayModel(
                status: status,
                message: message,
                detail: detail,
                audioLevel: audioLevel
            )
        }
        set {
            status = newValue.status
            message = newValue.message
            detail = newValue.detail
            audioLevel = newValue.audioLevel
        }
    }
}

struct OverlayEvent: Decodable {
    let type: String
    let text: String?
    let message: String?
    let level: Double?
    let status: String?
    let x: Double?

    var kind: OverlayEventType? {
        OverlayEventType(rawValue: type)
    }
}

struct VisualEffectView: NSViewRepresentable {
    var material: NSVisualEffectView.Material
    var blendingMode: NSVisualEffectView.BlendingMode
    var state: NSVisualEffectView.State

    func makeNSView(context: Context) -> NSVisualEffectView {
        let view = NSVisualEffectView()
        view.material = material
        view.blendingMode = blendingMode
        view.state = state
        return view
    }

    func updateNSView(_ nsView: NSVisualEffectView, context: Context) {
        nsView.material = material
        nsView.blendingMode = blendingMode
        nsView.state = state
    }
}

private struct CursorIndicator: View {
    let isTranscribing: Bool
    let size: CGFloat
    @State private var rotating = false

    var body: some View {
        ZStack {
            Circle()
                .fill(Color.white)

            if isTranscribing {
                // 9割だけ閉じたリング（1箇所欠け）を内側の縁に沿って回す
                Circle()
                    .inset(by: 1.2)
                    .trim(from: 0, to: 0.9)
                    .stroke(
                        Color(white: 0.4).opacity(0.9),
                        style: StrokeStyle(lineWidth: 1.5, lineCap: .round)
                    )
                    .rotationEffect(.degrees(rotating ? 360 : 0))
                    .onAppear { rotating = true }
                    .animation(.linear(duration: 1).repeatForever(autoreverses: false), value: rotating)
            }
        }
        .frame(width: size, height: size)
    }
}

// テキストの最終行末尾にインジケーターを追従させるビュー。
// CoreText でレイアウトを計算し、最終行の末尾 (x, y) にオーバーレイする。
private struct TextWithEndCursor: View {
    let text: String
    let fontSize: CGFloat
    let lineSpacing: CGFloat
    let isTranscribing: Bool
    let cursorSize: CGFloat

    @State private var cursorX: CGFloat = 0
    @State private var cursorY: CGFloat = 0

    var body: some View {
        Text(text)
            .font(.system(size: fontSize, weight: .semibold))
            .foregroundStyle(Color.white.opacity(0.96))
            .lineSpacing(lineSpacing)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(
                GeometryReader { geo in
                    Color.clear
                        .onAppear {
                            updateCursor(width: geo.size.width, height: geo.size.height)
                        }
                        .onChange(of: text) { _, _ in
                            updateCursor(width: geo.size.width, height: geo.size.height)
                        }
                        .onChange(of: geo.size.width) { _, newWidth in
                            updateCursor(width: newWidth, height: geo.size.height)
                        }
                        .onChange(of: geo.size.height) { _, newHeight in
                            updateCursor(width: geo.size.width, height: newHeight)
                        }
                }
            )
            .overlay(alignment: .topLeading) {
                CursorIndicator(isTranscribing: isTranscribing, size: cursorSize)
                    .offset(x: cursorX, y: cursorY)
            }
    }

    private func updateCursor(width: CGFloat, height: CGFloat) {
        let (x, y) = computeCursorPosition(text: text, width: width, height: height)
        cursorX = x
        cursorY = y
    }

    // CoreText で最終行末尾の座標 (x, y) を計算する。
    // y は SwiftUI の top-down 座標系で、テキストビュー上端からの距離。
    // CursorIndicator の中心が最終行の文字中心に揃うよう配置する。
    private func computeCursorPosition(text: String, width: CGFloat, height: CGFloat) -> (CGFloat, CGFloat) {
        guard !text.isEmpty, width > 0, height > 0 else { return (0, 0) }

        let nsFont = NSFont.systemFont(ofSize: fontSize, weight: .semibold)
        let paragraphStyle = NSMutableParagraphStyle()
        paragraphStyle.lineSpacing = lineSpacing

        let attrs: [NSAttributedString.Key: Any] = [
            .font: nsFont,
            .paragraphStyle: paragraphStyle,
        ]

        let attrStr = NSAttributedString(string: text, attributes: attrs)
        let framesetter = CTFramesetterCreateWithAttributedString(attrStr)
        let framePath = CGPath(
            rect: CGRect(x: 0, y: 0, width: width, height: 10_000),
            transform: nil
        )
        let ctFrame = CTFramesetterCreateFrame(
            framesetter, CFRange(location: 0, length: 0), framePath, nil
        )

        let lines = CTFrameGetLines(ctFrame) as! [CTLine]
        guard !lines.isEmpty else { return (0, 0) }

        let lastIdx = lines.count - 1
        let lastLine = lines[lastIdx]

        // X: 最終行の末尾文字位置 (末尾ホワイトスペースを除く)
        let fullWidth = CTLineGetTypographicBounds(lastLine, nil, nil, nil)
        let trailingWS = CTLineGetTrailingWhitespaceWidth(lastLine)
        let textEndX = fullWidth - trailingWS
        let x = min(textEndX + 4, width - cursorSize)

        // Y: 最終行のタイポグラフィ中心に CursorIndicator の中心を揃える。
        var ascent: CGFloat = 0
        var descent: CGFloat = 0
        _ = CTLineGetTypographicBounds(lastLine, &ascent, &descent, nil)
        let lastLineCenterY = height - ((ascent + descent) / 2)
        let rawY = lastLineCenterY - (cursorSize / 2)
        let y = min(max(rawY, 0), max(height - cursorSize, 0))

        return (x, y)
    }
}

struct OverlayView: View {
    @ObservedObject var state: OverlayState
    private let activeCornerRadius: CGFloat = 26
    private let horizontalPadding: CGFloat = 24
    private let fontSize: CGFloat = 20
    private let lineSpacing: CGFloat = 3
    private let verticalPaddingTop: CGFloat = 20
    private let verticalPaddingBottom: CGFloat = 20

    private var displayText: String {
        state.detail.isEmpty ? state.message : state.detail
    }

    private var textToken: String {
        displayText
    }

    private var cursorSize: CGFloat {
        fontSize * 0.72
    }

    private var textWithDot: some View {
        TextWithEndCursor(
            text: displayText,
            fontSize: fontSize,
            lineSpacing: lineSpacing,
            isTranscribing: state.status == "transcribing",
            cursorSize: cursorSize
        )
    }

    var body: some View {
        ZStack {
            if state.status != "idle" {
                VStack(spacing: 0) {
                    ScrollViewReader { proxy in
                        ScrollView(.vertical, showsIndicators: false) {
                            VStack(alignment: .leading, spacing: 0) {
                                textWithDot
                                    .multilineTextAlignment(.leading)
                                    .lineLimit(nil)
                                    .frame(maxWidth: .infinity, alignment: .topLeading)
                                Color.clear
                                    .frame(height: 1)
                                    .id("scroll-bottom")
                            }
                        }
                        .onChange(of: textToken) { _, _ in
                            DispatchQueue.main.async {
                                proxy.scrollTo("scroll-bottom", anchor: .bottom)
                            }
                        }
                        .onAppear {
                            DispatchQueue.main.async {
                                proxy.scrollTo("scroll-bottom", anchor: .bottom)
                            }
                        }
                    }
                    .padding(.horizontal, horizontalPadding)
                    .padding(.top, verticalPaddingTop)
                    .padding(.bottom, verticalPaddingBottom)
                }
                .background(
                    RoundedRectangle(cornerRadius: activeCornerRadius, style: .continuous)
                        .fill(Color.black.opacity(0.98))
                        .overlay(
                            RoundedRectangle(cornerRadius: activeCornerRadius, style: .continuous)
                                .stroke(Color.white.opacity(0.07), lineWidth: 1)
                        )
                )
            }
        }
    }
}

final class OverlayAppDelegate: NSObject, NSApplicationDelegate {
    private let state = OverlayState()
    private var panel: NSPanel?
    private var idleResetWorkItem: DispatchWorkItem?
    private let activeWidth: CGFloat = 600
    private let activeHeight: CGFloat = 150
    private let bottomMargin: CGFloat = 56
    private let topMargin: CGFloat = 12
#if NO_IDLE_OFFSET
    // ビルド時フラグでオフセットを無効化（初期表示のズレ対策）
    private let safetyOffsetIdleX: CGFloat = 0
#else
    // ステータス別のX補正量（idle:+55px, active:0px）
    private let safetyOffsetIdleX: CGFloat = 55
#endif
    private let safetyOffsetActiveX: CGFloat = 0

    func applicationDidFinishLaunching(_ notification: Notification) {
        createPanel()
        startStdinListener()
    }

    private func createPanel() {
        let contentView = NSHostingView(rootView: OverlayView(state: state))
        let panel = NSPanel(
            contentRect: NSRect(x: 0, y: 0, width: activeWidth, height: activeHeight),
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: false
        )

        panel.isFloatingPanel = true
        panel.level = .statusBar
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary, .ignoresCycle]
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.hasShadow = false
        panel.hidesOnDeactivate = false
        panel.ignoresMouseEvents = true
        panel.contentView = contentView
        prepareContentLayerAnchor()

        self.panel = panel
        positionPanel()
        hidePanel()
    }

    private func positionPanel(for size: NSSize? = nil) {
        guard let panel = panel else { return }
        let screen = panel.screen ?? NSScreen.main
        guard let screenFrame = screen?.visibleFrame else { return }
        let panelSize = size ?? panel.frame.size
        let offsetX = (state.status == "idle") ? safetyOffsetIdleX : safetyOffsetActiveX
        let centerX = screenFrame.origin.x + (screenFrame.width - panelSize.width) / 2
        let x = centerX + offsetX
        let y = screenFrame.minY + bottomMargin
        let frame = NSRect(origin: NSPoint(x: x, y: y), size: panelSize)
        panel.setFrame(frame, display: true)
        NSLog("floating-overlay positionPanel -> origin=(%.2f, %.2f) size=%.2fx%.2f desiredX=%@ offsetX=%.2f status=%@",
              frame.origin.x, frame.origin.y, frame.size.width, frame.size.height,
              "n/a",
              offsetX,
              state.status)
    }

    private func resizePanel() {
        guard let panel = panel else { return }
        let size: NSSize
        let maxHeight = availableMaxHeight()
        size = NSSize(width: activeWidth, height: min(activeHeight, maxHeight))
        panel.setFrame(NSRect(origin: panel.frame.origin, size: size), display: true)
        if let hostingView = panel.contentView as? NSHostingView<OverlayView> {
            hostingView.frame = NSRect(origin: .zero, size: size)
            hostingView.layoutSubtreeIfNeeded()
        }
        prepareContentLayerAnchor()
        positionPanel(for: size)
    }

    private func availableMaxHeight() -> CGFloat {
        guard let screen = NSScreen.main else { return activeHeight }
        let screenFrame = screen.visibleFrame
        return max(0, screenFrame.height - bottomMargin - topMargin)
    }

    private func hidePanel() {
        hidePanel(withAnimation: true)
    }

    private func hidePanel(withAnimation animated: Bool) {
        guard let panel = panel else { return }
        idleResetWorkItem?.cancel()
        guard let contentLayer = panel.contentView?.layer, animated else {
            panel.alphaValue = 1.0
            panel.orderOut(nil)
            return
        }

        NSAnimationContext.runAnimationGroup { context in
            context.duration = 0.12
            context.timingFunction = CAMediaTimingFunction(name: .easeInEaseOut)
            let scaleTransform = CATransform3DMakeScale(0.78, 0.62, 1.0)
            contentLayer.setAffineTransform(CATransform3DGetAffineTransform(scaleTransform))
        } completionHandler: {
            contentLayer.setAffineTransform(.identity)
            panel.orderOut(nil)
        }
    }

    private func showPanel() {
        guard let panel = panel else { return }
        if let layer = panel.contentView?.layer {
            layer.transform = CATransform3DIdentity
            layer.setAffineTransform(.identity)
        }
        resizePanel()
        positionPanel()
        panel.orderFrontRegardless()
    }

    /// Ensure bottom-center anchor so scale animations keep the bottom edge fixed.
    private func prepareContentLayerAnchor() {
        guard let contentView = panel?.contentView else { return }
        contentView.wantsLayer = true
        if let layer = contentView.layer {
            layer.anchorPoint = CGPoint(x: 0.5, y: 0.0)
            layer.position = CGPoint(x: contentView.bounds.midX, y: contentView.bounds.minY)
        }
    }

    private func startStdinListener() {
        DispatchQueue.global(qos: .background).async { [weak self] in
            while let line = readLine() {
                guard let data = line.data(using: .utf8) else { continue }
                if let event = try? JSONDecoder().decode(OverlayEvent.self, from: data) {
                    DispatchQueue.main.async {
                        self?.applyEvent(event)
                    }
                }
            }
        }
    }

    private func applyTransition(_ transition: OverlayTransition) {
        if transition.shouldCancelIdleReset {
            idleResetWorkItem?.cancel()
        }
        state.model = transition.model
        applyEffects(transition.effects)
    }

    private func applyEffects(_ effects: OverlayEffects) {
        if effects.shouldReposition {
            positionPanel(for: panel?.frame.size)
        }
        if effects.shouldResize {
            resizePanel()
        }
        if effects.shouldHide {
            hidePanel()
        } else if effects.shouldShow {
            showPanel()
        }
    }

    private func applyEvent(_ event: OverlayEvent) {
        let transition = OverlayEventReducer.reduce(model: state.model, event: event)
        applyTransition(transition)
    }
}
