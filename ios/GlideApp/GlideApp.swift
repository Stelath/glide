import SwiftUI

@main
struct GlideApp: App {
    @State private var settings = SettingsStore.shared
    @State private var liveSession = LiveDictationManager.shared
    @State private var showLiveSessionScreen = false

    var body: some Scene {
        WindowGroup {
            Group {
                if !settings.hasCompletedOnboarding {
                    OnboardingView()
                } else if showLiveSessionScreen {
                    LiveSessionActiveView(showLiveSessionScreen: $showLiveSessionScreen)
                } else {
                    ContentView()
                }
            }
            .animation(.easeInOut(duration: 0.3), value: settings.hasCompletedOnboarding)
            .animation(.easeInOut(duration: 0.3), value: showLiveSessionScreen)
            .environment(settings)
            .environment(liveSession)
            .tint(Color.glidePrimary)
            .onOpenURL { url in
                guard url.scheme == "glide", url.host == "start-session" else { return }
                guard settings.hasCompletedOnboarding else { return }

                if !liveSession.snapshot.phase.isActive {
                    liveSession.startSession()
                }
                showLiveSessionScreen = true
            }
            .onChange(of: liveSession.snapshot.phase) { _, newPhase in
                if !newPhase.isActive {
                    showLiveSessionScreen = false
                }
            }
        }
    }
}
