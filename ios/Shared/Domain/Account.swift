import Foundation

struct Account: Codable, Equatable, Sendable, Identifiable {
    var provider: AuthProvider
    var stableId: String
    var email: String?
    var displayName: String?
    var avatarURL: URL?
    var issuedAt: Date
    var expiresAt: Date?
    var subscriptionTier: SubscriptionTier
    var tokenUsage: TokenUsage

    var id: String { "\(provider.rawValue):\(stableId)" }

    var initials: String {
        let source = (displayName?.isEmpty == false ? displayName : email) ?? "?"
        let letters = source
            .split(whereSeparator: { !$0.isLetter })
            .prefix(2)
            .compactMap { $0.first.map(Character.init) }
        return letters.isEmpty ? "?" : String(letters).uppercased()
    }

    var shortDisplayName: String {
        if let name = displayName, !name.isEmpty { return name }
        if let email, let at = email.firstIndex(of: "@") { return String(email[..<at]) }
        return email ?? "Account"
    }
}
