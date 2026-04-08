import SwiftUI
import UIKit

private let settingsChangedNotification = CFNotificationName(SettingsStore.notificationName as CFString)

final class KeyboardViewController: UIInputViewController {
    private var hostingController: UIHostingController<AnyView>?

    override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        configureDictationBehavior()
    }

    override func viewDidLoad() {
        super.viewDidLoad()
        configureDictationBehavior()

        if let inputView = view as? UIInputView {
            inputView.allowsSelfSizing = true
        }

        SettingsStore.shared.reloadFromDisk()

        let keyboardView = KeyboardView(
            actions: KeyboardActions(
                deleteBackward: { [weak self] in
                    self?.textDocumentProxy.deleteBackward()
                },
                deleteWordBackward: { [weak self] in
                    self?.deleteWordBackward()
                },
                insertText: { [weak self] text in
                    self?.textDocumentProxy.insertText(text)
                },
                hasFullAccess: { [weak self] in
                    self?.hasFullAccess ?? false
                },
                openURL: { [weak self] url in
                    self?.openURL(url)
                }
            )
        )
        .environment(SettingsStore.shared)
        .environment(LiveDictationBridge.shared)

        let hostingController = UIHostingController(rootView: AnyView(keyboardView))
        hostingController.sizingOptions = .intrinsicContentSize
        hostingController.view.translatesAutoresizingMaskIntoConstraints = false
        hostingController.view.backgroundColor = .clear
        addChild(hostingController)
        view.addSubview(hostingController.view)

        NSLayoutConstraint.activate([
            hostingController.view.topAnchor.constraint(equalTo: view.topAnchor),
            hostingController.view.bottomAnchor.constraint(equalTo: view.bottomAnchor),
            hostingController.view.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            hostingController.view.trailingAnchor.constraint(equalTo: view.trailingAnchor),
        ])

        hostingController.didMove(toParent: self)
        self.hostingController = hostingController

        let center = CFNotificationCenterGetDarwinNotifyCenter()
        let observer = UnsafeMutableRawPointer(Unmanaged.passUnretained(self).toOpaque())
        CFNotificationCenterAddObserver(
            center,
            observer,
            { _, _, _, _, _ in
                Task { @MainActor in
                    SettingsStore.shared.reloadFromDisk()
                }
            },
            settingsChangedNotification.rawValue,
            nil,
            .deliverImmediately
        )
    }

    deinit {
        let observer = UnsafeMutableRawPointer(Unmanaged.passUnretained(self).toOpaque())
        CFNotificationCenterRemoveObserver(
            CFNotificationCenterGetDarwinNotifyCenter(),
            observer,
            settingsChangedNotification,
            nil
        )
    }

    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        updatePreferredHeight()
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        updatePreferredHeight()
    }

    private func configureDictationBehavior() {
        hasDictationKey = true
    }

    private func openURL(_ url: URL) {
        extensionContext?.open(url) { [weak self] success in
            guard !success else { return }
            Task { @MainActor in
                self?.openURLThroughResponderChain(url)
            }
        }
    }

    @MainActor
    private func openURLThroughResponderChain(_ url: URL) {
        let selector = NSSelectorFromString("openURL:")
        var responder: UIResponder? = self

        while let current = responder {
            if current.responds(to: selector) {
                _ = current.perform(selector, with: url)
                return
            }
            responder = current.next
        }
    }

    @MainActor
    private func deleteWordBackward() {
        let deleteCount = wordDeletionCount(in: textDocumentProxy.documentContextBeforeInput ?? "")
        for _ in 0..<deleteCount {
            textDocumentProxy.deleteBackward()
        }
    }

    private func wordDeletionCount(in context: String) -> Int {
        guard !context.isEmpty else { return 1 }

        let characters = Array(context)
        var index = characters.count - 1
        var deleteCount = 0

        while index >= 0, characters[index].isKeyboardWhitespace {
            deleteCount += 1
            index -= 1
        }
        if deleteCount > 0 {
            return deleteCount
        }

        while index >= 0, characters[index].isKeyboardDelimiter {
            deleteCount += 1
            index -= 1
        }
        if deleteCount > 0 {
            return deleteCount
        }

        while index >= 0, characters[index].isKeyboardWordCharacter {
            deleteCount += 1
            index -= 1
        }

        return max(1, deleteCount)
    }

    private func updatePreferredHeight() {
        let height = traitCollection.verticalSizeClass == .compact ? 160.0 : 240.0
        preferredContentSize = CGSize(width: view.bounds.width, height: height)
        view.invalidateIntrinsicContentSize()
    }
}

// MARK: - Actions

struct KeyboardActions: Sendable {
    let deleteBackward: @MainActor @Sendable () -> Void
    let deleteWordBackward: @MainActor @Sendable () -> Void
    let insertText: @MainActor @Sendable (String) -> Void
    let hasFullAccess: @MainActor @Sendable () -> Bool
    let openURL: @MainActor @Sendable (URL) -> Void
}

// MARK: - Keyboard View

struct KeyboardView: View {
    @Environment(SettingsStore.self) private var settings
    @Environment(LiveDictationBridge.self) private var liveSession
    @Environment(\.verticalSizeClass) private var verticalSizeClass

    let actions: KeyboardActions

    @State private var errorMessage: String?
    @State private var selectedStyleID: UUID?

    private var waveformAreaHeight: CGFloat {
        verticalSizeClass == .compact ? 96 : 136
    }

    private var waveformContentHeight: CGFloat {
        verticalSizeClass == .compact ? 60 : 88
    }

    var body: some View {
        VStack(spacing: 0) {
            // Main dictation area with style picker overlay
            waveformArea
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .overlay(alignment: .topTrailing) {
                    if !settings.styles.isEmpty {
                        styleMenuButton
                            .padding(.top, 6)
                            .padding(.trailing, 12)
                    }
                }

            // Bottom bar — status + action keys
            HStack(spacing: 0) {
                // Status indicator
                VStack(alignment: .leading, spacing: 2) {
                    if let errorMessage, !errorMessage.isEmpty {
                        Text(errorMessage)
                            .foregroundStyle(.red)
                    } else if liveSession.snapshot.phase == .stopping || liveSession.snapshot.phase == .processing {
                        Text("Processing...")
                            .foregroundStyle(.secondary)
                    }
                }
                .font(.caption2.weight(.medium))

                Spacer()

                HStack(spacing: 8) {
                    Button(action: insertReturn) {
                        Image(systemName: "return")
                            .font(.body.weight(.medium))
                            .foregroundStyle(.secondary)
                            .frame(width: 44, height: 44)
                            .background(Color(.tertiarySystemFill), in: RoundedRectangle(cornerRadius: 8))
                    }
                    .buttonStyle(.plain)

                    RepeatingKeyButton(action: deleteBackward, longPressAction: deleteWordBackward) {
                        Image(systemName: "delete.left")
                            .font(.body.weight(.medium))
                            .foregroundStyle(.secondary)
                            .frame(width: 44, height: 44)
                            .background(Color(.tertiarySystemFill), in: RoundedRectangle(cornerRadius: 8))
                    }
                }
            }
            .padding(.horizontal, 12)
            .padding(.bottom, 6)
            .padding(.top, 4)
        }
        // No explicit background so the keyboard integrates with the host app's chrome.
        .onChange(of: settings.styles) { _, newStyles in
            if let selectedStyleID, !newStyles.contains(where: { $0.id == selectedStyleID }) {
                self.selectedStyleID = nil
                liveSession.updateSelectedStyleID(nil)
            }
        }
        .onChange(of: liveSession.snapshot) { _, newSnapshot in
            syncLocalStyleSelection(with: newSnapshot)
            handleSnapshotChange(newSnapshot)
        }
        .onAppear {
            syncLocalStyleSelection(with: liveSession.snapshot)
            handleSnapshotChange(liveSession.snapshot)
        }
    }

    // MARK: - Style Menu

    private var selectedStyleLabel: String {
        if let id = selectedStyleID, let style = settings.styles.first(where: { $0.id == id }) {
            return style.name.isEmpty ? "Untitled" : style.name
        }
        return "Default"
    }

    private var styleMenuButton: some View {
        Menu {
            Button {
                selectedStyleID = nil
                liveSession.updateSelectedStyleID(nil)
            } label: {
                if selectedStyleID == nil {
                    Label("Default", systemImage: "checkmark")
                } else {
                    Text("Default")
                }
            }

            ForEach(Array(settings.styles.enumerated()), id: \.element.id) { _, style in
                Button {
                    selectedStyleID = style.id
                    liveSession.updateSelectedStyleID(style.id)
                } label: {
                    if selectedStyleID == style.id {
                        Label(style.name.isEmpty ? "Untitled" : style.name, systemImage: "checkmark")
                    } else {
                        Text(style.name.isEmpty ? "Untitled" : style.name)
                    }
                }
            }
        } label: {
            HStack(spacing: 4) {
                Text(selectedStyleLabel)
                    .font(.caption2.weight(.medium))
                Image(systemName: "chevron.down")
                    .font(.system(size: 8, weight: .bold))
            }
            .foregroundStyle(.secondary)
            .padding(.horizontal, 8)
            .padding(.vertical, 5)
            .background(Color(.tertiarySystemFill), in: Capsule())
        }
    }

    // MARK: - Waveform Area

    @ViewBuilder
    private var waveformArea: some View {
        ZStack {
            switch liveSession.snapshot.phase {
            case .idle, .ready, .completed, .failed:
                IdleWaveform(contentHeight: waveformContentHeight)
            case .recording:
                LiveSessionWaveform(contentHeight: waveformContentHeight)
            case .starting, .stopping, .processing:
                ProcessingAnimation(size: waveformContentHeight)
            }
        }
        .frame(maxWidth: .infinity)
        .frame(height: waveformContentHeight)
        .padding(.horizontal, 24)
        .padding(.vertical, 18)
        .frame(maxWidth: .infinity)
        .frame(height: waveformAreaHeight)
        .background(Color.black.opacity(0.001))
        .contentShape(Rectangle())
        .onTapGesture(perform: toggleLiveSession)
        .animation(.easeInOut(duration: 0.3), value: liveSession.snapshot.phase)
    }

    // MARK: - Live Session Logic

    @MainActor
    private func toggleLiveSession() {
        errorMessage = nil

        let snapshot = liveSession.snapshot

        switch snapshot.phase {
        case .starting, .stopping, .processing:
            return
        case .idle, .ready, .recording, .completed, .failed:
            break
        }

        if !snapshot.phase.isActive || snapshot.sessionID == nil {
            guard actions.hasFullAccess() else {
                errorMessage = "Enable Full Access in Settings"
                return
            }

            UIImpactFeedbackGenerator(style: .medium).impactOccurred()
            if let url = URL(string: "glide://start-session") {
                actions.openURL(url)
            }
            return
        }

        UIImpactFeedbackGenerator(style: .medium).impactOccurred()

        switch snapshot.phase {
        case .ready:
            liveSession.requestStart()

        case .recording:
            liveSession.requestStop()

        case .idle, .completed, .failed, .starting, .stopping, .processing:
            break
        }
    }

    @MainActor
    private func handleSnapshotChange(_ snapshot: LiveDictationSnapshot) {
        if let text = liveSession.consumeFinalText() {
            actions.insertText(text)
            errorMessage = nil
            UINotificationFeedbackGenerator().notificationOccurred(.success)
            return
        }

        if snapshot.phase == .failed {
            errorMessage = snapshot.errorMessage.isEmpty ? "Live session failed" : snapshot.errorMessage
            return
        }

        if snapshot.phase == .ready, !snapshot.errorMessage.isEmpty {
            errorMessage = snapshot.errorMessage
            return
        }

        if snapshot.phase != .idle {
            errorMessage = nil
        }
    }

    private func syncLocalStyleSelection(with snapshot: LiveDictationSnapshot) {
        if snapshot.phase.isActive {
            selectedStyleID = snapshot.selectedStyleID
        }
    }

    @MainActor
    private func insertReturn() {
        actions.insertText("\n")
    }

    @MainActor
    private func deleteBackward() {
        actions.deleteBackward()
    }

    @MainActor
    private func deleteWordBackward() {
        actions.deleteWordBackward()
    }
}

private struct RepeatingKeyButton<Label: View>: View {
    let action: @MainActor () -> Void
    let longPressAction: (@MainActor () -> Void)?
    let label: () -> Label

    @State private var isPressing = false
    @State private var repeatTimer: Timer?
    @State private var repeatStartWorkItem: DispatchWorkItem?
    @State private var wordDeleteWorkItem: DispatchWorkItem?

    init(
        action: @escaping @MainActor () -> Void,
        longPressAction: (@MainActor () -> Void)? = nil,
        @ViewBuilder label: @escaping () -> Label
    ) {
        self.action = action
        self.longPressAction = longPressAction
        self.label = label
    }

    var body: some View {
        label()
            .contentShape(Rectangle())
            .scaleEffect(isPressing ? 0.96 : 1.0)
            .animation(.easeOut(duration: 0.12), value: isPressing)
            .gesture(
                DragGesture(minimumDistance: 0)
                    .onChanged { _ in
                        beginPressIfNeeded()
                    }
                    .onEnded { _ in
                        endPress()
                    }
            )
            .onDisappear {
                endPress()
            }
    }

    private func beginPressIfNeeded() {
        guard !isPressing else { return }

        isPressing = true
        triggerAction()

        let workItem = DispatchWorkItem {
            startRepeating()
        }
        repeatStartWorkItem = workItem
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.45, execute: workItem)

        guard longPressAction != nil else { return }

        let wordWorkItem = DispatchWorkItem {
            switchToWordDeletionMode()
        }
        wordDeleteWorkItem = wordWorkItem
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.55, execute: wordWorkItem)
    }

    private func startRepeating() {
        guard isPressing else { return }
        let action = self.action

        repeatTimer?.invalidate()
        repeatTimer = Timer.scheduledTimer(withTimeInterval: 0.08, repeats: true) { _ in
            Task { @MainActor in action() }
        }
    }

    private func switchToWordDeletionMode() {
        guard isPressing, let longPressAction else { return }
        let action = longPressAction

        repeatTimer?.invalidate()
        repeatTimer = Timer.scheduledTimer(withTimeInterval: 0.16, repeats: true) { _ in
            Task { @MainActor in action() }
        }
    }

    private func endPress() {
        isPressing = false
        repeatStartWorkItem?.cancel()
        repeatStartWorkItem = nil
        wordDeleteWorkItem?.cancel()
        wordDeleteWorkItem = nil
        repeatTimer?.invalidate()
        repeatTimer = nil
    }

    private func triggerAction() {
        trigger(action)
    }

    private func trigger(_ action: @escaping @MainActor () -> Void) {
        Task { @MainActor in
            action()
        }
    }
}

private extension Character {
    private static let keyboardWordSeparatorScalars: Set<UInt32> = [39, 45, 95]

    var isKeyboardWhitespace: Bool {
        unicodeScalars.allSatisfy(CharacterSet.whitespacesAndNewlines.contains)
    }

    var isKeyboardWordCharacter: Bool {
        unicodeScalars.allSatisfy {
            CharacterSet.alphanumerics.contains($0) || Self.keyboardWordSeparatorScalars.contains($0.value)
        }
    }

    var isKeyboardDelimiter: Bool {
        !isKeyboardWhitespace && !isKeyboardWordCharacter
    }
}

// MARK: - Idle Waveform

private struct IdleWaveform: View {
    let contentHeight: CGFloat

    @State private var animating = false

    private let barCount = 40

    private var minHeight: CGFloat {
        max(6, contentHeight * 0.08)
    }

    private var maxHeight: CGFloat {
        contentHeight * 0.82
    }

    var body: some View {
        HStack(spacing: 3) {
            ForEach(0..<barCount, id: \.self) { index in
                RoundedRectangle(cornerRadius: 1.5)
                    .fill(GlideAccent.current.primary.opacity(0.3))
                    .frame(width: 4, height: idleHeight(for: index))
                    .animation(
                        .easeInOut(duration: 1.8)
                            .repeatForever(autoreverses: true)
                            .delay(Double(index) * 0.025),
                        value: animating
                    )
            }
        }
        .frame(maxWidth: .infinity)
        .frame(height: contentHeight)
        .clipped()
        .onAppear {
            animating = true
        }
    }

    private func idleHeight(for index: Int) -> CGFloat {
        let normalized = Double(index) / Double(barCount - 1)
        let wave = sin(normalized * .pi)
        let expandedHeight = minHeight + (maxHeight - minHeight) * wave
        return animating ? expandedHeight : minHeight
    }
}

// MARK: - Live Session Waveform

private struct LiveSessionWaveform: View {
    let contentHeight: CGFloat

    @State private var pulse = false

    private let barCount = 40

    private var minHeight: CGFloat {
        max(10, contentHeight * 0.14)
    }

    private var maxHeight: CGFloat {
        contentHeight * 0.95
    }

    var body: some View {
        HStack(spacing: 3) {
            ForEach(0..<barCount, id: \.self) { index in
                RoundedRectangle(cornerRadius: 2)
                    .fill(barColor(for: index))
                    .frame(width: 4, height: barHeight(for: index))
                    .animation(
                        .easeInOut(duration: 0.9)
                            .repeatForever(autoreverses: true)
                            .delay(Double(index % 8) * 0.035),
                        value: pulse
                    )
            }
        }
        .frame(maxWidth: .infinity)
        .frame(height: contentHeight)
        .clipped()
        .onAppear {
            pulse = true
        }
    }

    private func barHeight(for index: Int) -> CGFloat {
        let normalized = Double(index) / Double(barCount - 1)
        let envelope = sin(normalized * .pi)
        let envelopeHeight = minHeight + (maxHeight - minHeight) * envelope
        let animatedOffset = pulse ? 16.0 : 6.0
        let ripple = sin(normalized * .pi * 5 + (pulse ? .pi / 2 : 0))
        let height = envelopeHeight + animatedOffset * abs(ripple)
        return max(minHeight, min(maxHeight, height))
    }

    private func barColor(for index: Int) -> Color {
        let normalized = Double(index) / Double(barCount - 1)
        let intensity = sin(normalized * .pi)
        return Color.red.opacity(0.45 + 0.55 * intensity)
    }
}

// MARK: - Processing Animation

private struct ProcessingAnimation: View {
    let size: CGFloat

    @State private var rotation: Double = 0
    @State private var morphed = false

    private let dotCount = 16

    var body: some View {
        ZStack {
            ForEach(0..<dotCount, id: \.self) { index in
                let angle = Double(index) / Double(dotCount) * 360.0

                Circle()
                    .fill(GlideAccent.current.primary.opacity(dotOpacity(for: index)))
                    .frame(width: 6, height: 6)
                    .offset(x: morphed ? size * 0.33 : spreadX(for: index))
                    .rotationEffect(.degrees(angle + rotation))
            }
        }
        .frame(width: size, height: size)
        .onAppear {
            withAnimation(.easeInOut(duration: 0.4)) {
                morphed = true
            }
            withAnimation(.linear(duration: 1.2).repeatForever(autoreverses: false)) {
                rotation = 360
            }
        }
    }

    private func spreadX(for index: Int) -> CGFloat {
        let normalized = Double(index) / Double(dotCount - 1)
        let halfWidth = size * 1.15
        return CGFloat(normalized) * (halfWidth * 2) - halfWidth
    }

    private func dotOpacity(for index: Int) -> Double {
        let normalized = Double(index) / Double(dotCount)
        return 0.3 + 0.7 * (1.0 - normalized)
    }
}
