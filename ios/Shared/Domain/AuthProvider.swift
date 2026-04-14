import Foundation

enum AuthProvider: String, Codable, Sendable, CaseIterable {
    case apple
    case google

    var displayName: String {
        switch self {
        case .apple: "Apple"
        case .google: "Google"
        }
    }
}
