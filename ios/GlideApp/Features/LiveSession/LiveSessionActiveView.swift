import SwiftUI

struct LiveSessionActiveView: View {
    @Environment(SettingsStore.self) private var settings
    @Environment(LiveDictationManager.self) private var liveSession
    @Binding var showLiveSessionScreen: Bool

    @State private var appeared = false

    var body: some View {
        ZStack {
            LinearGradient(
                colors: [Color.glideBackground, settings.accent.accentSurface.opacity(0.3)],
                startPoint: .top,
                endPoint: .bottom
            )
            .ignoresSafeArea()

            VStack(spacing: 32) {
                Spacer()

                PulsingMicIcon()
                    .frame(height: 160)

                VStack(spacing: 12) {
                    Text("Glide")
                        .font(.largeTitle.weight(.bold))

                    Text(statusText)
                        .font(.callout.weight(.medium))
                        .foregroundStyle(statusTint)
                }
                .opacity(appeared ? 1 : 0)
                .offset(y: appeared ? 0 : 16)

                SwipeRightAnimation()
                    .frame(height: 60)
                    .opacity(appeared ? 1 : 0)
                    .offset(y: appeared ? 0 : 12)

                Spacer()

                Button(role: .destructive) {
                    liveSession.cancelSession()
                } label: {
                    Label("End Session", systemImage: "xmark.circle.fill")
                        .fontWeight(.medium)
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(.bordered)
                .tint(.red)
                .controlSize(.large)
                .padding(.horizontal, 32)
                .opacity(appeared ? 1 : 0)

                // Bottom swipe hint bar
                HStack(spacing: 8) {
                    Image(systemName: "hand.point.right.fill")
                        .font(.callout)
                    Text("Swipe right to return to your app")
                        .font(.callout.weight(.medium))
                }
                .foregroundStyle(.secondary)
                .padding(.horizontal, 20)
                .padding(.vertical, 10)
                .background(Color.glideSurface, in: Capsule())
                .opacity(appeared ? 1 : 0)
                .padding(.bottom, 8)
            }
        }
        .overlay(alignment: .topTrailing) {
            Button {
                showLiveSessionScreen = false
            } label: {
                Image(systemName: "xmark.circle.fill")
                    .font(.title)
                    .symbolRenderingMode(.hierarchical)
                    .foregroundStyle(.secondary)
            }
            .padding(20)
            .opacity(appeared ? 1 : 0)
        }
        .onAppear {
            withAnimation(.easeOut(duration: 0.6).delay(0.15)) {
                appeared = true
            }
        }
    }

    private var statusText: String {
        switch liveSession.snapshot.phase {
        case .starting:
            return "Starting..."
        case .ready:
            return "Microphone active"
        case .recording:
            return "Capturing snippet"
        case .stopping, .processing:
            return "Processing..."
        default:
            return "Session active"
        }
    }

    private var statusTint: Color {
        switch liveSession.snapshot.phase {
        case .recording:
            return .red
        case .ready:
            return settings.accent.primary
        case .stopping, .processing:
            return settings.accent.primary
        default:
            return .secondary
        }
    }
}
