import SwiftUI
import WidgetKit

@main
struct GlideLiveActivityBundle: WidgetBundle {
    var body: some Widget {
        GlideLiveActivityWidget()
    }
}

struct GlideLiveActivityWidget: Widget {
    var body: some WidgetConfiguration {
        ActivityConfiguration(for: LiveSessionAttributes.self) { context in
            // Lock Screen banner
            LockScreenLiveActivityView(context: context)
        } dynamicIsland: { context in
            DynamicIsland {
                // Expanded regions
                DynamicIslandExpandedRegion(.leading) {
                    Image(systemName: "mic.fill")
                        .font(.title2)
                        .foregroundStyle(context.state.phase == "recording" ? .red : Color.glidePrimary)
                }
                DynamicIslandExpandedRegion(.center) {
                    VStack(spacing: 2) {
                        Text("Glide")
                            .font(.headline)
                        if context.state.phase != "ready" {
                            Text(phaseLabel(context.state.phase))
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                    }
                }
                DynamicIslandExpandedRegion(.trailing) {
                    Button(intent: EndLiveSessionIntent()) {
                        Image(systemName: "xmark.circle.fill")
                            .font(.title2)
                            .foregroundStyle(.red)
                    }
                    .buttonStyle(.plain)
                }
                DynamicIslandExpandedRegion(.bottom) {
                    if context.state.snippetCount > 0 {
                        Text("\(context.state.snippetCount) snippet\(context.state.snippetCount == 1 ? "" : "s") captured")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                }
            } compactLeading: {
                Image(systemName: "mic.fill")
                    .foregroundStyle(context.state.phase == "recording" ? .red : Color.glidePrimary)
            } compactTrailing: {
                if context.state.phase != "ready" {
                    Text(phaseLabel(context.state.phase))
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                }
            } minimal: {
                Image(systemName: "mic.fill")
                    .foregroundStyle(context.state.phase == "recording" ? .red : Color.glidePrimary)
            }
        }
    }
}

private func phaseLabel(_ phase: String) -> String {
    switch phase {
    case "recording":
        return "Recording"
    case "processing":
        return "Processing"
    case "ready":
        return "Listening"
    default:
        return "Active"
    }
}
