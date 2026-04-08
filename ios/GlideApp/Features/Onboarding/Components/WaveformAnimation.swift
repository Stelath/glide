import SwiftUI

struct WaveformAnimation: View {
    @Environment(SettingsStore.self) private var settings
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
            let normalized = Double(index) / Double(barCount - 1)
            let envelope = sin(normalized * .pi)
            return minHeight + (maxHeight - minHeight) * envelope * Double.random(in: 0.3...1.0)
        } else {
            return minHeight
        }
    }

    private func barColor(for index: Int) -> Color {
        let normalized = Double(index) / Double(barCount - 1)
        return settings.accent.primary.opacity(0.3 + 0.7 * sin(normalized * .pi))
    }
}
