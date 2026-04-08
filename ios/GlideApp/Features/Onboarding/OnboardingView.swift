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
            .tint(settings.accent.primary)
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
                .tint(settings.accent.primary)
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
