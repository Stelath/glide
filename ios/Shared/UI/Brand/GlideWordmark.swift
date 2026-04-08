import SwiftUI

/// Vertical bars positioned at different offsets to spell "glide" in lowercase.
/// Audio visualizer effect: each segment scales upward and downward dynamically from its exact center.
struct GlideWordmark: View {
    @Environment(SettingsStore.self) private var settings
    private static let columns: [[(bottom: CGFloat, top: CGFloat)]] = buildColumns()

    private let frameHeight: CGFloat = 40
    private let barWidth: CGFloat = 4.0
    private let barSpacing: CGFloat = 2.0
    private let gapWidth: CGFloat = 6.0

    var body: some View {
        TimelineView(.animation(minimumInterval: 1.0 / 60.0)) { timeline in
            let now = timeline.date.timeIntervalSinceReferenceDate
            HStack(alignment: .bottom, spacing: barSpacing) {
                ForEach(0..<Self.columns.count, id: \.self) { col in
                    let segments = Self.columns[col]
                    if segments.isEmpty {
                        Spacer().frame(width: gapWidth)
                    } else {
                        ZStack(alignment: .bottom) {
                            ForEach(0..<segments.count, id: \.self) { seg in
                                let segment = segments[seg]
                                let primaryWave = sin(now * 6.0 + Double(col) * 1.5)
                                let secondaryWave = cos(now * 9.0 + Double(seg) * 3.1)
                                let combinedWave = (primaryWave * 0.6) + (secondaryWave * 0.4)
                                let maxExpansion: CGFloat = 4.0
                                let expansion = CGFloat(combinedWave) * maxExpansion
                                let originalHeight = (segment.top - segment.bottom) * frameHeight
                                let height = max(2, originalHeight + expansion)
                                let centerOffset = (segment.bottom * frameHeight) - ((height - originalHeight) / 2)

                                RoundedRectangle(cornerRadius: barWidth / 2)
                                    .fill(settings.accent.primary.opacity(0.4 + 0.6 * segment.top))
                                    .frame(width: barWidth, height: height)
                                    .offset(y: -centerOffset)
                            }
                        }
                        .frame(height: frameHeight, alignment: .bottom)
                    }
                }
            }
        }
    }

    private static func buildColumns() -> [[(bottom: CGFloat, top: CGFloat)]] {
        let g: [[(bottom: CGFloat, top: CGFloat)]] = [
            [(0.10, 0.20), (0.30, 0.68)],
            [(0.04, 0.10), (0.20, 0.30), (0.68, 0.78)],
            [(0.04, 0.10), (0.20, 0.30), (0.68, 0.78)],
            [(0.04, 0.78)],
        ]

        let l: [[(bottom: CGFloat, top: CGFloat)]] = [
            [(0.15, 1.00)]
        ]

        let i: [[(bottom: CGFloat, top: CGFloat)]] = [
            [(0.15, 0.70), (0.85, 0.95)]
        ]

        let d: [[(bottom: CGFloat, top: CGFloat)]] = [
            [(0.25, 0.60)],
            [(0.15, 0.25), (0.60, 0.70)],
            [(0.15, 0.25), (0.60, 0.70)],
            [(0.15, 1.00)]
        ]

        let e: [[(bottom: CGFloat, top: CGFloat)]] = [
            [(0.20, 0.65)],
            [(0.15, 0.25), (0.45, 0.50), (0.65, 0.70)],
            [(0.15, 0.25), (0.45, 0.50), (0.65, 0.70)],
            [(0.45, 0.65)]
        ]

        let gap: [[(bottom: CGFloat, top: CGFloat)]] = [[]]
        return g + gap + l + gap + i + gap + d + gap + e
    }
}
