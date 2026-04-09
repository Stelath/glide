import SwiftUI
import WidgetKit

struct LockScreenLiveActivityView: View {
    let context: ActivityViewContext<LiveSessionAttributes>

    var body: some View {
        HStack(spacing: 16) {
            // Mic indicator
            ZStack {
                Circle()
                    .fill(phaseTint.opacity(0.15))
                    .frame(width: 44, height: 44)

                Image(systemName: "mic.fill")
                    .font(.title3.weight(.medium))
                    .foregroundStyle(phaseTint)
            }

            // Status
            VStack(alignment: .leading, spacing: 2) {
                Text("Glide Live Session")
                    .font(.subheadline.weight(.semibold))

                HStack(spacing: 6) {
                    if context.state.phase == "recording" {
                        Circle()
                            .fill(GlideAccent.current.primary)
                            .frame(width: 6, height: 6)
                    }

                    Text(statusText)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }

            Spacer()

            // End button
            Button(intent: EndLiveSessionIntent()) {
                Image(systemName: "xmark.circle.fill")
                    .font(.title2)
                    .foregroundStyle(.secondary)
            }
            .buttonStyle(.plain)
        }
        .padding(16)
        .activityBackgroundTint(GlideAccent.current.accentSurface)
    }

    private var statusText: String {
        switch context.state.phase {
        case "recording":
            return "Capturing snippet"
        case "processing":
            return "Processing..."
        case "ready":
            if context.state.snippetCount > 0 {
                return "\(context.state.snippetCount) snippet\(context.state.snippetCount == 1 ? "" : "s") captured"
            }
            return "Microphone active"
        default:
            return "Session active"
        }
    }

    private var phaseTint: Color {
        GlideAccent.current.primary
    }
}
