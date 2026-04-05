import ActivityKit
import AppIntents
import Foundation

// MARK: - Activity Attributes

struct LiveSessionAttributes: ActivityAttributes {
    struct ContentState: Codable, Hashable {
        var phase: String
        var snippetCount: Int
    }

    var startedAt: Date
}

// MARK: - End Session Intent (used by Live Activity button)

struct EndLiveSessionIntent: LiveActivityIntent {
    static let title: LocalizedStringResource = "End Live Session"
    static let description: IntentDescription = "Ends the Glide live dictation session"

    func perform() async throws -> some IntentResult {
        await MainActor.run {
            LiveDictationBridge.shared.requestCancel()
        }
        return .result()
    }
}
