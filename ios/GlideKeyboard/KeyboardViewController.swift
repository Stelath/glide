import SwiftUI
import UIKit

private let settingsChangedNotification = CFNotificationName(SettingsStore.notificationName as CFString)

final class KeyboardViewController: UIInputViewController {
    private var hostingController: UIHostingController<AnyView>?

    override func viewDidLoad() {
        super.viewDidLoad()

        if let inputView = view as? UIInputView {
            inputView.allowsSelfSizing = true
        }

        SettingsStore.shared.reloadFromDisk()

        let keyboardView = KeyboardView(
            actions: KeyboardActions(
                deleteBackward: { [weak self] in
                    self?.textDocumentProxy.deleteBackward()
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

    private func openURL(_ url: URL) {
        extensionContext?.open(url, completionHandler: nil)
    }

    private func updatePreferredHeight() {
        let height = traitCollection.verticalSizeClass == .compact ? 120.0 : 160.0
        preferredContentSize = CGSize(width: view.bounds.width, height: height)
        view.invalidateIntrinsicContentSize()
    }
}

// MARK: - Actions

struct KeyboardActions: Sendable {
    let deleteBackward: @MainActor @Sendable () -> Void
    let insertText: @MainActor @Sendable (String) -> Void
    let hasFullAccess: @MainActor @Sendable () -> Bool
    let openURL: @MainActor @Sendable (URL) -> Void
}

// MARK: - Keyboard View

struct KeyboardView: View {
    @Environment(SettingsStore.self) private var settings
    @Environment(LiveDictationBridge.self) private var liveSession

    let actions: KeyboardActions

    @State private var errorMessage: String?
    @State private var selectedStyleID: UUID?

    var body: some View {
        VStack(spacing: 0) {
            // Style pills — compact, top edge
            if !settings.styles.isEmpty {
                styleBar
                    .padding(.top, 4)
                    .padding(.bottom, 2)
            }

            // Main dictation area
            waveformArea
                .frame(maxWidth: .infinity, maxHeight: .infinity)

            // Bottom bar — status + delete
            HStack {
                // Status indicator
                Group {
                    if let errorMessage, !errorMessage.isEmpty {
                        Text(errorMessage)
                            .foregroundStyle(.red)
                    } else if liveSession.snapshot.phase == .recording {
                        HStack(spacing: 5) {
                            Circle()
                                .fill(.red)
                                .frame(width: 6, height: 6)
                            Text("Recording")
                        }
                        .foregroundStyle(.secondary)
                    } else if liveSession.snapshot.phase == .stopping || liveSession.snapshot.phase == .processing {
                        Text("Processing...")
                            .foregroundStyle(.secondary)
                    }
                }
                .font(.caption2.weight(.medium))

                Spacer()

                if liveSession.snapshot.phase == .ready || liveSession.snapshot.phase == .recording {
                    Button(action: endLiveSession) {
                        Image(systemName: "xmark.circle")
                            .font(.body.weight(.medium))
                            .foregroundStyle(.secondary)
                            .frame(width: 32, height: 32)
                    }
                    .buttonStyle(.plain)
                }

                Button(action: deleteBackward) {
                    Image(systemName: "delete.left")
                        .font(.body.weight(.medium))
                        .foregroundStyle(.secondary)
                        .frame(width: 32, height: 32)
                }
                .buttonStyle(.plain)
            }
            .padding(.horizontal, 16)
            .padding(.bottom, 4)
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

    // MARK: - Style Bar

    private var styleBar: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 6) {
                miniPill(title: "Default", dotColor: .accentColor, isSelected: selectedStyleID == nil) {
                    selectedStyleID = nil
                    liveSession.updateSelectedStyleID(nil)
                }

                ForEach(Array(settings.styles.enumerated()), id: \.element.id) { index, style in
                    miniPill(
                        title: style.name.isEmpty ? "Untitled" : style.name,
                        dotColor: Self.pillColors[index % Self.pillColors.count],
                        isSelected: selectedStyleID == style.id
                    ) {
                        selectedStyleID = style.id
                        liveSession.updateSelectedStyleID(style.id)
                    }
                }
            }
            .padding(.horizontal, 16)
        }
    }

    private static let pillColors: [Color] = [.blue, .purple, .orange, .pink, .teal, .indigo]

    @ViewBuilder
    private func miniPill(title: String, dotColor: Color, isSelected: Bool, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            HStack(spacing: 4) {
                Circle()
                    .fill(isSelected ? .white : dotColor)
                    .frame(width: 5, height: 5)

                Text(title)
                    .font(.caption2.weight(.medium))
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 5)
            .background(isSelected ? Color.accentColor : Color(uiColor: .tertiarySystemFill))
            .foregroundStyle(isSelected ? .white : .secondary)
            .clipShape(Capsule())
        }
        .buttonStyle(.plain)
    }

    // MARK: - Waveform Area

    @ViewBuilder
    private var waveformArea: some View {
        Button {
            toggleLiveSession()
        } label: {
            ZStack {
                switch liveSession.snapshot.phase {
                case .idle, .ready, .completed, .failed:
                    IdleWaveform()
                case .recording:
                    LiveSessionWaveform()
                case .starting, .stopping, .processing:
                    ProcessingAnimation()
                }
            }
            .padding(.horizontal, 24)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .disabled(liveSession.snapshot.phase == .stopping || liveSession.snapshot.phase == .processing)
        .animation(.easeInOut(duration: 0.3), value: liveSession.snapshot.phase)
    }

    // MARK: - Live Session Logic

    @MainActor
    private func toggleLiveSession() {
        errorMessage = nil
        UIImpactFeedbackGenerator(style: .medium).impactOccurred()

        switch liveSession.snapshot.phase {
        case .idle, .completed, .failed:
            guard actions.hasFullAccess() else {
                errorMessage = "Enable Full Access in Settings"
                return
            }

            if let url = URL(string: "glide://start-session") {
                actions.openURL(url)
            }

        case .ready:
            liveSession.requestStart()

        case .recording:
            liveSession.requestStop()

        case .starting, .stopping, .processing:
            break
        }
    }

    @MainActor
    private func endLiveSession() {
        errorMessage = nil
        liveSession.requestCancel()
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
    private func deleteBackward() {
        actions.deleteBackward()
    }
}

// MARK: - Idle Waveform

private struct IdleWaveform: View {
    @State private var animating = false

    private let barCount = 40

    var body: some View {
        HStack(spacing: 3) {
            ForEach(0..<barCount, id: \.self) { index in
                RoundedRectangle(cornerRadius: 1.5)
                    .fill(Color.accentColor.opacity(0.3))
                    .frame(width: 4, height: animating ? idleHeight(for: index) : 4)
            }
        }
        .onAppear {
            withAnimation(.easeInOut(duration: 2.0).repeatForever(autoreverses: true)) {
                animating = true
            }
        }
    }

    private func idleHeight(for index: Int) -> CGFloat {
        let normalized = Double(index) / Double(barCount - 1)
        let wave = sin(normalized * .pi)
        return 4 + 8 * wave
    }
}

// MARK: - Live Session Waveform

private struct LiveSessionWaveform: View {
    @State private var pulse = false

    private let barCount = 40

    var body: some View {
        HStack(spacing: 3) {
            ForEach(0..<barCount, id: \.self) { index in
                RoundedRectangle(cornerRadius: 2)
                    .fill(barColor(for: index))
                    .frame(width: 4, height: barHeight(for: index))
            }
        }
        .onAppear {
            withAnimation(.easeInOut(duration: 0.9).repeatForever(autoreverses: true)) {
                pulse = true
            }
        }
    }

    private func barHeight(for index: Int) -> CGFloat {
        let normalized = Double(index) / Double(barCount - 1)
        let envelope = sin(normalized * .pi)
        let animatedOffset = pulse ? 10.0 : 4.0
        let ripple = sin(normalized * .pi * 5 + (pulse ? .pi / 2 : 0))
        let height = 8 + 32 * envelope + animatedOffset * abs(ripple)
        return max(8, min(56, height))
    }

    private func barColor(for index: Int) -> Color {
        let normalized = Double(index) / Double(barCount - 1)
        let intensity = sin(normalized * .pi)
        return Color.red.opacity(0.45 + 0.55 * intensity)
    }
}

// MARK: - Processing Animation

private struct ProcessingAnimation: View {
    @State private var rotation: Double = 0
    @State private var morphed = false

    private let dotCount = 16

    var body: some View {
        ZStack {
            ForEach(0..<dotCount, id: \.self) { index in
                let angle = Double(index) / Double(dotCount) * 360.0

                Circle()
                    .fill(Color.accentColor.opacity(dotOpacity(for: index)))
                    .frame(width: 6, height: 6)
                    .offset(x: morphed ? 20 : spreadX(for: index))
                    .rotationEffect(.degrees(angle + rotation))
            }
        }
        .frame(width: 60, height: 60)
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
        return CGFloat(normalized * 140 - 70)
    }

    private func dotOpacity(for index: Int) -> Double {
        let normalized = Double(index) / Double(dotCount)
        return 0.3 + 0.7 * (1.0 - normalized)
    }
}
