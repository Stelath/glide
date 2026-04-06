import AVFoundation
import SwiftUI
import UIKit

struct OnboardingView: View {
    @Environment(SettingsStore.self) private var settings
    @Environment(\.openURL) private var openURL

    @State private var currentStep = 0
    @State private var micPermission = AVAudioApplication.shared.recordPermission
    @State private var stepAppeared = false

    private let totalSteps = 4

    var body: some View {
        ZStack {
            // Background gradient
            LinearGradient(
                colors: [Color.glideBackground, Color.glideSurface],
                startPoint: .top,
                endPoint: .bottom
            )
            .ignoresSafeArea()

            VStack(spacing: 0) {
                TabView(selection: $currentStep) {
                    welcomeStep.tag(0)
                    addKeyboardStep.tag(1)
                    fullAccessStep.tag(2)
                    microphoneStep.tag(3)
                }
                .tabViewStyle(.page(indexDisplayMode: .always))

                navigationButtons
                    .padding(.horizontal, 24)
                    .padding(.bottom, 40)
            }
        }
        .onChange(of: currentStep) { _, _ in
            stepAppeared = false
            withAnimation(.easeOut(duration: 0.5).delay(0.15)) {
                stepAppeared = true
            }
        }
        .onAppear {
            withAnimation(.easeOut(duration: 0.6).delay(0.2)) {
                stepAppeared = true
            }
        }
    }

    // MARK: - Step 0: Welcome

    private var welcomeStep: some View {
        VStack(spacing: 24) {
            Spacer()
            Spacer()

            WaveformAnimation()
                .frame(height: 120)
                .padding(.horizontal, 40)

            VStack(spacing: 12) {
                Text("Welcome to Glide")
                    .font(.largeTitle.weight(.bold))

                Text("Voice dictation powered by your own API keys.\nStart a live session in Glide, then return to the keyboard anywhere.")
                    .font(.body)
                    .foregroundStyle(.secondary)
                    .multilineTextAlignment(.center)
                    .padding(.horizontal, 24)
            }
            .opacity(stepAppeared ? 1 : 0)
            .offset(y: stepAppeared ? 0 : 16)

            Spacer()
            Spacer()
            Spacer()
        }
        .padding(.horizontal, 24)
    }

    // MARK: - Step 1: Add Keyboard

    private var addKeyboardStep: some View {
        VStack(spacing: 24) {
            Spacer()
            Spacer()

            SettingsPathAnimation()
                .frame(height: 200)
                .padding(.horizontal, 24)

            VStack(spacing: 12) {
                Text("Add Glide Keyboard")
                    .font(.title.weight(.bold))

                Text("Follow the path in Settings to add Glide as a keyboard.")
                    .font(.body)
                    .foregroundStyle(.secondary)
                    .multilineTextAlignment(.center)
                    .padding(.horizontal, 16)
            }
            .opacity(stepAppeared ? 1 : 0)
            .offset(y: stepAppeared ? 0 : 16)

            Button {
                if let url = URL(string: UIApplication.openSettingsURLString) {
                    openURL(url)
                }
            } label: {
                Label("Open Settings", systemImage: "gear")
                    .fontWeight(.medium)
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.bordered)
            .tint(Color.glidePrimary)
            .controlSize(.large)
            .padding(.horizontal, 32)
            .opacity(stepAppeared ? 1 : 0)

            Spacer()
            Spacer()
            Spacer()
        }
        .padding(.horizontal, 24)
    }

    // MARK: - Step 2: Full Access

    private var fullAccessStep: some View {
        VStack(spacing: 24) {
            Spacer()
            Spacer()

            ToggleAnimation()
                .frame(height: 160)

            VStack(spacing: 12) {
                Text("Enable Full Access")
                    .font(.title.weight(.bold))

                Text("Glide needs Full Access to make network calls for speech-to-text and LLM processing.\n\nYour API keys stay securely on-device.")
                    .font(.body)
                    .foregroundStyle(.secondary)
                    .multilineTextAlignment(.center)
                    .padding(.horizontal, 16)
            }
            .opacity(stepAppeared ? 1 : 0)
            .offset(y: stepAppeared ? 0 : 16)

            Spacer()
            Spacer()
            Spacer()
        }
        .padding(.horizontal, 24)
    }

    // MARK: - Step 3: Microphone

    private var microphoneStep: some View {
        VStack(spacing: 24) {
            Spacer()
            Spacer()

            MicrophoneAnimation(granted: micPermission == .granted)
                .frame(height: 160)

            VStack(spacing: 12) {
                Text("Microphone Access")
                    .font(.title.weight(.bold))

                if micPermission == .granted {
                    Label("Microphone access granted", systemImage: "checkmark.circle.fill")
                        .font(.body.weight(.medium))
                        .foregroundStyle(.green)
                } else {
                    Text("Glide needs microphone access so the app can run live dictation sessions for the keyboard.")
                        .font(.body)
                        .foregroundStyle(.secondary)
                        .multilineTextAlignment(.center)
                        .padding(.horizontal, 16)
                }
            }
            .opacity(stepAppeared ? 1 : 0)
            .offset(y: stepAppeared ? 0 : 16)

            if micPermission != .granted {
                Button {
                    if micPermission == .undetermined {
                        AVAudioApplication.requestRecordPermission { granted in
                            Task { @MainActor in
                                withAnimation(.spring(duration: 0.4)) {
                                    micPermission = granted ? .granted : .denied
                                }
                            }
                        }
                    } else if let url = URL(string: UIApplication.openSettingsURLString) {
                        openURL(url)
                    }
                } label: {
                    Label(
                        micPermission == .undetermined ? "Allow Microphone" : "Open Settings",
                        systemImage: micPermission == .undetermined ? "mic.fill" : "gear"
                    )
                    .fontWeight(.medium)
                    .frame(maxWidth: .infinity)
                }
                .buttonStyle(.bordered)
                .tint(Color.glidePrimary)
                .controlSize(.large)
                .padding(.horizontal, 32)
                .opacity(stepAppeared ? 1 : 0)
            }

            Spacer()
            Spacer()
            Spacer()
        }
        .padding(.horizontal, 24)
    }

    // MARK: - Navigation

    private var navigationButtons: some View {
        HStack {
            if currentStep > 0 {
                Button {
                    withAnimation { currentStep -= 1 }
                } label: {
                    Image(systemName: "chevron.left")
                        .font(.body.weight(.semibold))
                        .frame(width: 44, height: 44)
                }
                .foregroundStyle(.secondary)
            }

            Spacer()

            if currentStep < totalSteps - 1 {
                Button {
                    withAnimation { currentStep += 1 }
                } label: {
                    Text(currentStep == 0 ? "Get Started" : "Next")
                        .fontWeight(.semibold)
                        .padding(.horizontal, 8)
                }
                .buttonStyle(.borderedProminent)
                .controlSize(.large)
            } else {
                Button {
                    settings.hasCompletedOnboarding = true
                } label: {
                    Text("Done")
                        .fontWeight(.semibold)
                        .padding(.horizontal, 8)
                }
                .buttonStyle(.borderedProminent)
                .controlSize(.large)
            }
        }
    }
}

// MARK: - Waveform Animation (Welcome)

private struct WaveformAnimation: View {
    @State private var animating = false

    private let barCount = 24
    private let minHeight: CGFloat = 8
    private let maxHeight: CGFloat = 60

    var body: some View {
        HStack(spacing: 4) {
            ForEach(0..<barCount, id: \.self) { index in
                RoundedRectangle(cornerRadius: 2)
                    .fill(barColor(for: index))
                    .frame(width: 6, height: barHeight(for: index))
                    .animation(
                        .easeInOut(duration: Double.random(in: 0.4...0.8))
                        .repeatForever(autoreverses: true)
                        .delay(Double(index) * 0.05),
                        value: animating
                    )
            }
        }
        .onAppear {
            animating = true
        }
    }

    private func barHeight(for index: Int) -> CGFloat {
        if animating {
            // Create a wave pattern that varies per bar
            let normalized = Double(index) / Double(barCount - 1)
            let envelope = sin(normalized * .pi) // Taller in the middle
            return minHeight + (maxHeight - minHeight) * envelope * Double.random(in: 0.3...1.0)
        } else {
            return minHeight
        }
    }

    private func barColor(for index: Int) -> Color {
        let normalized = Double(index) / Double(barCount - 1)
        return Color.glidePrimary.opacity(0.3 + 0.7 * sin(normalized * .pi))
    }
}

// MARK: - Settings Path Animation (Add Keyboard)

private struct SettingsPathAnimation: View {
    @State private var highlightedRow = -1

    private let rows = [
        ("gear", "Settings"),
        ("switch.2", "General"),
        ("keyboard", "Keyboard"),
        ("plus.circle.fill", "Add — Glide"),
    ]

    var body: some View {
        VStack(spacing: 0) {
            ForEach(Array(rows.enumerated()), id: \.offset) { index, row in
                HStack(spacing: 14) {
                    Image(systemName: row.0)
                        .font(.body.weight(.medium))
                        .foregroundStyle(index == highlightedRow ? .white : .secondary)
                        .frame(width: 30, height: 30)
                        .background(
                            RoundedRectangle(cornerRadius: 7)
                                .fill(index == highlightedRow ? Color.glidePrimary : Color.glideAccentSurface)
                        )

                    Text(row.1)
                        .font(.body.weight(index == highlightedRow ? .semibold : .regular))
                        .foregroundStyle(index == highlightedRow ? Color.glideText : .secondary)

                    Spacer()

                    if index < rows.count - 1 {
                        Image(systemName: "chevron.right")
                            .font(.caption.weight(.semibold))
                            .foregroundStyle(.tertiary)
                    } else {
                        Image(systemName: "checkmark")
                            .font(.caption.weight(.bold))
                            .foregroundStyle(index == highlightedRow ? Color.glidePrimary : .clear)
                    }
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 12)
                .background(
                    RoundedRectangle(cornerRadius: 10)
                        .fill(index == highlightedRow ? Color.glideAccentSurface : .clear)
                )

                if index < rows.count - 1 {
                    Divider()
                        .padding(.leading, 60)
                }
            }
        }
        .padding(4)
        .background(
            RoundedRectangle(cornerRadius: 14)
                .fill(Color.glideSurface)
        )
        .onAppear { startAnimation() }
    }

    private func startAnimation() {
        // Cycle through highlighting each row
        func cycle() {
            for i in 0..<rows.count {
                DispatchQueue.main.asyncAfter(deadline: .now() + Double(i) * 0.8) {
                    withAnimation(.easeInOut(duration: 0.3)) {
                        highlightedRow = i
                    }
                }
            }
            // Hold on last row, then restart
            DispatchQueue.main.asyncAfter(deadline: .now() + Double(rows.count) * 0.8 + 1.5) {
                withAnimation(.easeInOut(duration: 0.2)) {
                    highlightedRow = -1
                }
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) {
                    cycle()
                }
            }
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) {
            cycle()
        }
    }
}

// MARK: - Toggle Animation (Full Access)

private struct ToggleAnimation: View {
    @State private var isOn = false

    var body: some View {
        VStack(spacing: 20) {
            // Mock settings row
            HStack(spacing: 14) {
                Image(systemName: "keyboard")
                    .font(.body.weight(.medium))
                    .foregroundStyle(.white)
                    .frame(width: 30, height: 30)
                    .background(
                        RoundedRectangle(cornerRadius: 7)
                            .fill(Color.glidePrimary)
                    )

                Text("Allow Full Access")
                    .font(.body)

                Spacer()

                // Animated toggle
                Capsule()
                    .fill(isOn ? Color.green : Color.glideAccentSurface)
                    .frame(width: 51, height: 31)
                    .overlay(alignment: isOn ? .trailing : .leading) {
                        Circle()
                            .fill(.white)
                            .shadow(color: .black.opacity(0.15), radius: 2, y: 1)
                            .frame(width: 27, height: 27)
                            .padding(2)
                    }
                    .animation(.spring(duration: 0.35, bounce: 0.15), value: isOn)
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 12)
            .background(
                RoundedRectangle(cornerRadius: 14)
                    .fill(Color.glideSurface)
            )
            .padding(.horizontal, 8)

            // Animated hand tap indicator
            if !isOn {
                Image(systemName: "hand.tap.fill")
                    .font(.title2)
                    .foregroundStyle(.secondary)
                    .symbolEffect(.pulse, options: .repeating)
            } else {
                Image(systemName: "checkmark.circle.fill")
                    .font(.title2)
                    .foregroundStyle(.green)
                    .transition(.scale.combined(with: .opacity))
            }
        }
        .onAppear { startAnimation() }
    }

    private func startAnimation() {
        func cycle() {
            DispatchQueue.main.asyncAfter(deadline: .now() + 1.2) {
                withAnimation { isOn = true }
            }
            DispatchQueue.main.asyncAfter(deadline: .now() + 3.5) {
                withAnimation { isOn = false }
            }
            DispatchQueue.main.asyncAfter(deadline: .now() + 4.5) {
                cycle()
            }
        }
        cycle()
    }
}

// MARK: - Microphone Animation

private struct MicrophoneAnimation: View {
    let granted: Bool

    @State private var pulseScale: CGFloat = 1.0
    @State private var ringOpacity: Double = 0.6

    var body: some View {
        ZStack {
            if granted {
                // Success state
                Circle()
                    .fill(Color.green.opacity(0.12))
                    .frame(width: 120, height: 120)

                Image(systemName: "checkmark.circle.fill")
                    .font(.system(size: 64, weight: .light))
                    .foregroundStyle(.green)
                    .transition(.scale.combined(with: .opacity))
            } else {
                // Pulsing rings
                ForEach(0..<3, id: \.self) { ring in
                    Circle()
                        .stroke(Color.glidePrimary.opacity(ringOpacity * (1.0 - Double(ring) * 0.3)), lineWidth: 2)
                        .frame(
                            width: 80 + CGFloat(ring) * 30 * pulseScale,
                            height: 80 + CGFloat(ring) * 30 * pulseScale
                        )
                }

                // Mic icon
                Circle()
                    .fill(Color.glidePrimary.opacity(0.15))
                    .frame(width: 80, height: 80)

                Image(systemName: "mic.fill")
                    .font(.system(size: 36, weight: .medium))
                    .foregroundStyle(Color.glidePrimary)
            }
        }
        .onAppear {
            guard !granted else { return }
            withAnimation(.easeInOut(duration: 1.5).repeatForever(autoreverses: true)) {
                pulseScale = 1.3
                ringOpacity = 0.15
            }
        }
        .animation(.spring(duration: 0.5), value: granted)
    }
}
