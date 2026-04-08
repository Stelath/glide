import SwiftUI

struct SettingsPathAnimation: View {
    @Environment(SettingsStore.self) private var settings
    @State private var highlightedRow = -1

    private let rows = [
        ("gear", "Settings"),
        ("switch.2", "General"),
        ("keyboard", "Keyboard"),
        ("plus.circle.fill", "Add - Glide"),
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
                                .fill(index == highlightedRow ? settings.accent.primary : settings.accent.accentSurface)
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
                            .foregroundStyle(index == highlightedRow ? settings.accent.primary : .clear)
                    }
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 12)
                .background(
                    RoundedRectangle(cornerRadius: 10)
                        .fill(index == highlightedRow ? settings.accent.accentSurface : .clear)
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
        .onAppear {
            startAnimation()
        }
    }

    private func startAnimation() {
        func cycle() {
            for index in 0..<rows.count {
                DispatchQueue.main.asyncAfter(deadline: .now() + Double(index) * 0.8) {
                    withAnimation(.easeInOut(duration: 0.3)) {
                        highlightedRow = index
                    }
                }
            }

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
