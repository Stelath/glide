import SwiftUI

struct StyleCard: View {
    let style: DictationStyle
    let accentColor: Color
    let summary: String

    var body: some View {
        HStack(spacing: 12) {
            RoundedRectangle(cornerRadius: 2)
                .fill(accentColor)
                .frame(width: 4, height: 40)

            VStack(alignment: .leading, spacing: 4) {
                Text(style.name.isEmpty ? "Untitled" : style.name)
                    .font(.body.weight(.medium))

                Text(summary)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
            }
        }
        .padding(.vertical, 2)
    }
}
