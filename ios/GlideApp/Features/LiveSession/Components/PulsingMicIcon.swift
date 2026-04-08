import SwiftUI

struct PulsingMicIcon: View {
    @Environment(SettingsStore.self) private var settings
    @State private var pulseScale: CGFloat = 1.0
    @State private var ringOpacity: Double = 0.5

    var body: some View {
        ZStack {
            ForEach(0..<3, id: \.self) { ring in
                Circle()
                    .stroke(settings.accent.primary.opacity(ringOpacity * (1.0 - Double(ring) * 0.25)), lineWidth: 2)
                    .frame(
                        width: 80 + CGFloat(ring) * 30 * pulseScale,
                        height: 80 + CGFloat(ring) * 30 * pulseScale
                    )
            }

            Circle()
                .fill(settings.accent.primary.opacity(0.15))
                .frame(width: 80, height: 80)

            Image(systemName: "mic.fill")
                .font(.system(size: 36, weight: .medium))
                .foregroundStyle(settings.accent.primary)
        }
        .onAppear {
            withAnimation(.easeInOut(duration: 1.5).repeatForever(autoreverses: true)) {
                pulseScale = 1.3
                ringOpacity = 0.12
            }
        }
    }
}
