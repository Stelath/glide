import AVFoundation
import SwiftUI
import UIKit

struct GeneralView: View {
    @Environment(SettingsStore.self) private var settings
    @Environment(\.openURL) private var openURL

    @State private var micPermission = AVAudioApplication.shared.recordPermission

    var body: some View {
        NavigationStack {
            VStack(spacing: 0) {
                HStack {
                    Text("General")
                        .font(.subheadline.weight(.semibold))
                        .foregroundStyle(Color.glideText)
                }
                .frame(maxWidth: .infinity)
                .padding(.vertical, 10)

                Divider()

                Form {
                Section("Appearance") {
                    HStack(spacing: 16) {
                        Spacer()
                        ForEach(GlideAccent.allCases) { accent in
                            VStack(spacing: 6) {
                                Circle()
                                    .fill(accent.primary)
                                    .frame(width: 36, height: 36)
                                    .overlay(
                                        Circle()
                                            .stroke(Color.glideText, lineWidth: settings.accent == accent ? 2.5 : 0)
                                            .padding(-3)
                                    )
                                Text(accent.displayName)
                                    .font(.caption2)
                                    .foregroundStyle(settings.accent == accent ? Color.glideText : .secondary)
                            }
                            .onTapGesture {
                                withAnimation(.easeInOut(duration: 0.2)) {
                                    settings.accent = accent
                                }
                                Task {
                                    try? await UIApplication.shared.setAlternateIconName(accent.iconName)
                                }
                            }
                        }
                        Spacer()
                    }
                    .padding(.vertical, 4)
                }

                Section("Microphone") {
                    LabeledContent("Status") {
                        HStack(spacing: 6) {
                            Image(systemName: micPermission == .granted ? "checkmark.circle.fill" : "xmark.circle.fill")
                                .foregroundStyle(micPermission == .granted ? .green : .red)
                            Text(microphoneStatusText)
                        }
                    }

                    if micPermission != .granted {
                        Button(micPermission == .undetermined ? "Request Permission" : "Open Settings") {
                            if micPermission == .undetermined {
                                AVAudioApplication.requestRecordPermission { granted in
                                    Task { @MainActor in
                                        micPermission = granted ? .granted : .denied
                                    }
                                }
                            } else if let url = URL(string: UIApplication.openSettingsURLString) {
                                openURL(url)
                            }
                        }
                    }
                }

                Section("Keyboard") {
                    Button("Re-run Setup Guide") {
                        settings.hasCompletedOnboarding = false
                    }
                }

                Section("About") {
                    LabeledContent("App Version", value: appVersion)
                }
            }
            }
            .toolbar(.hidden, for: .navigationBar)
            .background(Color.glideBackground)
            .scrollContentBackground(.hidden)
        }
        .onAppear {
            micPermission = AVAudioApplication.shared.recordPermission
        }
    }

    private var microphoneStatusText: String {
        switch micPermission {
        case .granted:
            return "Allowed"
        case .denied:
            return "Denied"
        case .undetermined:
            return "Not requested"
        @unknown default:
            return "Unknown"
        }
    }

    private var appVersion: String {
        let shortVersion = Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "-"
        let build = Bundle.main.object(forInfoDictionaryKey: "CFBundleVersion") as? String ?? "-"
        return "\(shortVersion) (\(build))"
    }
}
