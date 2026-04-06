import Foundation

struct ProviderInfo: Identifiable, Hashable, Sendable {
    let id: String
    let displayName: String
    let defaultBaseURL: String
    let symbolName: String
    let logoAssetName: String

    static let openAI = ProviderInfo(
        id: "openai",
        displayName: "OpenAI",
        defaultBaseURL: "https://api.openai.com/v1",
        symbolName: "sparkles",
        logoAssetName: "ProviderOpenAI"
    )

    static let groq = ProviderInfo(
        id: "groq",
        displayName: "Groq",
        defaultBaseURL: "https://api.groq.com/openai/v1",
        symbolName: "bolt.circle",
        logoAssetName: "ProviderGroq"
    )

    static let all: [ProviderInfo] = [.openAI, .groq]

    static func info(for providerID: String) -> ProviderInfo {
        all.first(where: { $0.id == providerID.lowercased() }) ?? .openAI
    }
}

struct DictationStyle: Codable, Identifiable, Hashable, Sendable {
    var id: UUID = UUID()
    var name: String
    var prompt: String
    var sttProvider: String? = nil
    var sttModel: String? = nil
    var llmProvider: String? = nil
    var llmModel: String? = nil
}

struct ModelOption: Hashable, Sendable {
    let provider: String
    let model: String

    var displayName: String {
        "\(model) (\(ProviderInfo.info(for: provider).displayName))"
    }
}
