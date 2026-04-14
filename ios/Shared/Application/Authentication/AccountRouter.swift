import Foundation

enum APIRouting: Sendable {
    case userProvidedKeys(openAI: String, openAIBaseURL: String, groq: String, groqBaseURL: String)
    case subscription(bearer: String, baseURL: String)
}

@MainActor
struct AccountRouter {
    let settings: SettingsStore
    let account: AccountStore

    static var shared: AccountRouter {
        AccountRouter(settings: .shared, account: .shared)
    }

    func currentRouting() -> APIRouting {
        .userProvidedKeys(
            openAI: settings.openAIApiKey,
            openAIBaseURL: settings.openAIBaseURL,
            groq: settings.groqApiKey,
            groqBaseURL: settings.groqBaseURL
        )
    }
}
