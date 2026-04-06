import SwiftUI

struct LiveSessionActiveView: View {
    @Environment(LiveDictationManager.self) private var liveSession
    @Binding var showLiveSessionScreen: Bool

    @State private var appeared = false

    var body: some View {
        ZStack {
            LinearGradient(
                colors: [Color.glideBackground, Color.glideSurface],
                startPoint: .top,
                endPoint: .bottom
            )
            .ignoresSafeArea()

            VStack(spacing: 32) {
                Spacer()

                PulsingMicIcon()
                    .frame(height: 160)

                VStack(spacing: 12) {
                    Text("Live Session Active")
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
                    .font(.title2)
                    .symbolRenderingMode(.hierarchical)
                    .foregroundStyle(.secondary)
            }
            .padding(16)
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
            return .glidePrimary
        case .stopping, .processing:
            return .glidePrimary
        default:
            return .secondary
        }
    }
}

// MARK: - Pulsing Mic Icon

private struct PulsingMicIcon: View {
    @State private var pulseScale: CGFloat = 1.0
    @State private var ringOpacity: Double = 0.5

    var body: some View {
        ZStack {
            ForEach(0..<3, id: \.self) { ring in
                Circle()
                    .stroke(Color.glidePrimary.opacity(ringOpacity * (1.0 - Double(ring) * 0.25)), lineWidth: 2)
                    .frame(
                        width: 80 + CGFloat(ring) * 30 * pulseScale,
                        height: 80 + CGFloat(ring) * 30 * pulseScale
                    )
            }

            Circle()
                .fill(Color.glidePrimary.opacity(0.15))
                .frame(width: 80, height: 80)

            Image(systemName: "mic.fill")
                .font(.system(size: 36, weight: .medium))
                .foregroundStyle(Color.glidePrimary)
        }
        .onAppear {
            withAnimation(.easeInOut(duration: 1.5).repeatForever(autoreverses: true)) {
                pulseScale = 1.3
                ringOpacity = 0.12
            }
        }
    }
}

// MARK: - Swipe Right Animation

private struct SwipeRightAnimation: View {
    @State private var offset: CGFloat = -40
    @State private var opacity: Double = 0

    var body: some View {
        ZStack {
            // Track line
            Capsule()
                .fill(.secondary.opacity(0.15))
                .frame(width: 120, height: 4)

            // Animated hand
            Image(systemName: "hand.point.right.fill")
                .font(.title2)
                .foregroundStyle(.secondary)
                .offset(x: offset)
                .opacity(opacity)
        }
        .onAppear {
            startAnimation()
        }
    }

    private func startAnimation() {
        offset = -40
        opacity = 0

        withAnimation(.easeIn(duration: 0.3)) {
            opacity = 1
        }

        withAnimation(.easeInOut(duration: 1.0).delay(0.2)) {
            offset = 40
        }

        withAnimation(.easeOut(duration: 0.3).delay(1.0)) {
            opacity = 0
        }

        // Repeat
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.8) {
            startAnimation()
        }
    }
}
