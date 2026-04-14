import Foundation

enum SubscriptionTier: String, Codable, Sendable, CaseIterable {
    case unsubscribed
    case free
    case pro

    var displayName: String {
        switch self {
        case .unsubscribed: "None"
        case .free: "Free"
        case .pro: "Pro"
        }
    }
}

struct TokenUsage: Codable, Equatable, Sendable {
    var transcriptionSeconds: Int
    var llmTokensIn: Int
    var llmTokensOut: Int

    static let zero = TokenUsage(transcriptionSeconds: 0, llmTokensIn: 0, llmTokensOut: 0)
}
