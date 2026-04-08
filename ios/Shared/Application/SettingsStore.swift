import Foundation
import Observation
import Security

enum VerificationState: Equatable, Sendable {
    case idle
    case verifying
    case verified
    case failed(String)
}

@MainActor
@Observable
final class SettingsStore {
    static let shared = SettingsStore()

    static let suiteName = "group.com.stelath.glide.app"
    nonisolated static let notificationName = "com.glide.settings-changed"

    @ObservationIgnored
    private let store: AppGroupKeyValueStore

    @ObservationIgnored
    private var isReloading = false

    @ObservationIgnored
    private var openAIVerifyTask: Task<Void, Never>?

    @ObservationIgnored
    private var groqVerifyTask: Task<Void, Never>?

    // MARK: - Provider Config

    var openAIApiKey: String { didSet { persistSecret(openAIApiKey, service: "glide-openai"); openAIVerification = .idle } }
    var openAIBaseURL: String { didSet { persistString(openAIBaseURL, key: "openai_base_url"); openAIVerification = .idle } }
    var groqApiKey: String { didSet { persistSecret(groqApiKey, service: "glide-groq"); groqVerification = .idle } }
    var groqBaseURL: String { didSet { persistString(groqBaseURL, key: "groq_base_url"); groqVerification = .idle } }

    // MARK: - Model Selection

    var sttProvider: String { didSet { persistString(sttProvider, key: "stt_provider") } }
    var sttModel: String { didSet { persistString(sttModel, key: "stt_model") } }
    var llmEnabled: Bool { true }
    var llmProvider: String { didSet { persistString(llmProvider, key: "llm_provider") } }
    var llmModel: String { didSet { persistString(llmModel, key: "llm_model") } }
    var systemPrompt: String { didSet { persistString(systemPrompt, key: "system_prompt") } }
    var styles: [DictationStyle] { didSet { persistStyles(styles) } }

    // MARK: - Appearance

    var accent: GlideAccent { didSet { persistString(accent.rawValue, key: "accent") } }

    // MARK: - Onboarding

    var hasCompletedOnboarding: Bool { didSet { persistBool(hasCompletedOnboarding, key: "has_completed_onboarding") } }

    // MARK: - Verification State

    var openAIVerification: VerificationState = .idle
    var groqVerification: VerificationState = .idle

    var openAIVerified: Bool { openAIVerification == .verified }
    var groqVerified: Bool { groqVerification == .verified }

    var sttModelsByProvider: [String: [String]] = [:]
    var llmModelsByProvider: [String: [String]] = [:]

    // MARK: - Unified Model Lists

    var allSTTModels: [ModelOption] {
        ProviderInfo.all.flatMap { provider in
            sttModels(for: provider.id).map { ModelOption(provider: provider.id, model: $0) }
        }
    }

    var allLLMModels: [ModelOption] {
        ProviderInfo.all.flatMap { provider in
            llmModels(for: provider.id).map { ModelOption(provider: provider.id, model: $0) }
        }
    }

    // MARK: - Init

    private init() {
        store = AppGroupKeyValueStore(appGroupIdentifier: Self.suiteName, fileName: "settings.store")

        openAIApiKey = ""
        openAIBaseURL = ProviderInfo.openAI.defaultBaseURL
        groqApiKey = ""
        groqBaseURL = ProviderInfo.groq.defaultBaseURL
        sttProvider = ""
        sttModel = ""
        llmProvider = ""
        llmModel = ""
        systemPrompt = Self.defaultSystemPrompt
        styles = Self.defaultStyles
        accent = .purple
        hasCompletedOnboarding = false

        reloadFromDisk()
    }

    /// Preview-only initializer — does not read from disk.
    init(preview: Bool) {
        store = AppGroupKeyValueStore(appGroupIdentifier: Self.suiteName, fileName: "settings.store")
        openAIApiKey = ""
        openAIBaseURL = ProviderInfo.openAI.defaultBaseURL
        groqApiKey = ""
        groqBaseURL = ProviderInfo.groq.defaultBaseURL
        sttProvider = ""
        sttModel = ""
        llmProvider = ""
        llmModel = ""
        systemPrompt = Self.defaultSystemPrompt
        styles = Self.defaultStyles
        accent = .purple
        hasCompletedOnboarding = false
    }

    // MARK: - Reload

    func reloadFromDisk() {
        isReloading = true
        openAIApiKey = readSecret(service: "glide-openai")
        openAIBaseURL = store.string(forKey: "openai_base_url") ?? ProviderInfo.openAI.defaultBaseURL
        groqApiKey = readSecret(service: "glide-groq")
        groqBaseURL = store.string(forKey: "groq_base_url") ?? ProviderInfo.groq.defaultBaseURL
        sttProvider = store.string(forKey: "stt_provider") ?? ""
        sttModel = store.string(forKey: "stt_model") ?? ""
        llmProvider = store.string(forKey: "llm_provider") ?? ""
        llmModel = store.string(forKey: "llm_model") ?? ""
        systemPrompt = store.string(forKey: "system_prompt") ?? Self.defaultSystemPrompt
        styles = readStyles()
        accent = GlideAccent(rawValue: store.string(forKey: "accent") ?? "") ?? .purple
        hasCompletedOnboarding = store.bool(forKey: "has_completed_onboarding")
        isReloading = false
    }

    // MARK: - Provider Helpers

    func apiKey(for provider: String) -> String {
        normalizedProvider(provider) == ProviderInfo.groq.id ? groqApiKey : openAIApiKey
    }

    func baseURL(for provider: String) -> String {
        normalizedProvider(provider) == ProviderInfo.groq.id ? groqBaseURL : openAIBaseURL
    }

    func sttModels(for provider: String) -> [String] {
        let normalized = normalizedProvider(provider)
        return sttModelsByProvider[normalized] ?? []
    }

    func llmModels(for provider: String) -> [String] {
        llmModelsByProvider[normalizedProvider(provider)] ?? []
    }

    func verificationState(for provider: String) -> VerificationState {
        normalizedProvider(provider) == ProviderInfo.groq.id ? groqVerification : openAIVerification
    }

    // MARK: - Model Updates

    func updateModels(for provider: String, result: FetchModelsResult) {
        let normalized = normalizedProvider(provider)
        sttModelsByProvider[normalized] = result.stt.sorted()
        llmModelsByProvider[normalized] = result.llm.sorted()

        if normalized == ProviderInfo.openAI.id {
            openAIVerification = .verified
        } else {
            groqVerification = .verified
        }

        if sttProvider == normalized, (sttModel.isEmpty || !sttModels(for: normalized).contains(sttModel)), let first = result.stt.first {
            sttModel = first
        }

        if llmProvider == normalized, (llmModel.isEmpty || !llmModels(for: normalized).contains(llmModel)), let first = result.llm.first {
            llmModel = first
        }
    }

    func setVerificationState(_ state: VerificationState, for provider: String) {
        let normalized = normalizedProvider(provider)
        if normalized == ProviderInfo.openAI.id {
            openAIVerification = state
        } else {
            groqVerification = state
        }
    }

    // MARK: - Auto-Verify (debounced)

    func scheduleAutoVerify(for provider: String) {
        let normalized = normalizedProvider(provider)
        if normalized == ProviderInfo.groq.id {
            groqVerifyTask?.cancel()
            groqVerifyTask = Task { @MainActor in
                try? await Task.sleep(for: .milliseconds(800))
                guard !Task.isCancelled else { return }
                await performVerification(for: normalized)
            }
        } else {
            openAIVerifyTask?.cancel()
            openAIVerifyTask = Task { @MainActor in
                try? await Task.sleep(for: .milliseconds(800))
                guard !Task.isCancelled else { return }
                await performVerification(for: normalized)
            }
        }
    }

    func performVerification(for provider: String) async {
        let normalized = normalizedProvider(provider)
        let key = apiKey(for: normalized).trimmingCharacters(in: .whitespacesAndNewlines)

        guard !key.isEmpty else {
            setVerificationState(.idle, for: normalized)
            return
        }

        setVerificationState(.verifying, for: normalized)

        do {
            let result = try await GlideCoreBridge.shared.fetchModels(
                apiKey: apiKey(for: normalized),
                baseURL: baseURL(for: normalized)
            )
            updateModels(for: normalized, result: result)
        } catch {
            setVerificationState(.failed(error.localizedDescription), for: normalized)
        }
    }

    // MARK: - Persistence

    private func persistString(_ value: String, key: String) {
        guard !isReloading else { return }
        store.set(value, forKey: key)
        postSettingsChangedNotification()
    }

    private func persistBool(_ value: Bool, key: String) {
        guard !isReloading else { return }
        store.set(value, forKey: key)
        postSettingsChangedNotification()
    }

    private func persistStyles(_ value: [DictationStyle]) {
        guard !isReloading else { return }
        if let data = try? JSONEncoder().encode(value) {
            store.set(data, forKey: "styles")
            postSettingsChangedNotification()
        }
    }

    private func readStyles() -> [DictationStyle] {
        guard let data = store.data(forKey: "styles"),
              let decoded = try? JSONDecoder().decode([DictationStyle].self, from: data)
        else {
            return Self.defaultStyles
        }
        return decoded
    }

    private func persistSecret(_ value: String, service: String) {
        guard !isReloading else { return }
        writeSecret(service: service, value: value)
        postSettingsChangedNotification()
    }

    private func readSecret(service: String) -> String {
        for accessGroup in [Self.suiteName, nil] {
            let query = secretQuery(service: service, accessGroup: accessGroup, returnData: true)
            var result: AnyObject?
            let status = SecItemCopyMatching(query as CFDictionary, &result)
            if status == errSecSuccess,
               let data = result as? Data,
               let value = String(data: data, encoding: .utf8) {
                return value
            }
        }

        return store.string(forKey: secretFallbackKey(for: service)) ?? ""
    }

    private func writeSecret(service: String, value: String) {
        store.removeObject(forKey: secretFallbackKey(for: service))

        for accessGroup in [Self.suiteName, nil] {
            let baseQuery = secretQuery(service: service, accessGroup: accessGroup, returnData: false)
            SecItemDelete(baseQuery as CFDictionary)

            guard !value.isEmpty else {
                continue
            }

            var addQuery = baseQuery
            addQuery[kSecValueData as String] = value.data(using: .utf8)
            let status = SecItemAdd(addQuery as CFDictionary, nil)
            if status == errSecSuccess {
                return
            }
        }

        if !value.isEmpty {
            store.set(value, forKey: secretFallbackKey(for: service))
        }
    }

    private func secretQuery(service: String, accessGroup: String?, returnData: Bool) -> [String: Any] {
        var query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: service,
        ]
        if let accessGroup {
            query[kSecAttrAccessGroup as String] = accessGroup
        }
        if returnData {
            query[kSecReturnData as String] = true
            query[kSecMatchLimit as String] = kSecMatchLimitOne
        }
        return query
    }

    private func secretFallbackKey(for service: String) -> String {
        "fallback_\(service)"
    }

    private func normalizedProvider(_ provider: String) -> String {
        provider.lowercased() == ProviderInfo.groq.id ? ProviderInfo.groq.id : ProviderInfo.openAI.id
    }

    private func postSettingsChangedNotification() {
        let center = CFNotificationCenterGetDarwinNotifyCenter()
        let name = CFNotificationName(Self.notificationName as CFString)
        CFNotificationCenterPostNotification(center, name, nil, nil, true)
    }
}

extension SettingsStore {
    static let defaultSystemPrompt = """
    You are a dictation post-processor. You receive raw speech-to-text output and return clean text ready to be typed into an application.

    Your job:
    - Remove filler words (um, uh, you know, like) unless they carry meaning.
    - Fix spelling, grammar, and punctuation errors.
    - When the transcript already contains a word that is a close misspelling of a name or term from the context or custom vocabulary, correct the spelling. Never insert names or terms from context that the speaker did not say.
    - Preserve the speaker's intent, tone, and meaning exactly.

    Output rules:
    - Return ONLY the cleaned transcript text, nothing else.
    - If the transcription is empty, return exactly: EMPTY
    - Do not add words, names, or content that are not in the transcription. The context is only for correcting spelling of words already spoken.
    - Do not change the meaning of what was said.
    """

    static let defaultStyles: [DictationStyle] = [
        DictationStyle(
            name: "Professional",
            prompt: """
            You are a dictation post-processor for professional communication. You receive raw speech-to-text output and return clean, formal text ready to be typed into a work application.

            Your job:
            - Remove filler words (um, uh, you know, like) unless they carry meaning.
            - Fix spelling, grammar, and punctuation errors.
            - Elevate the language to a professional, clear, and well-structured tone.
            - When the transcript already contains a word that is a close misspelling of a name or term from the context, correct the spelling. Never insert names or terms the speaker did not say.
            - Preserve the speaker's intent and meaning exactly.

            Output rules:
            - Return ONLY the cleaned transcript text, nothing else.
            - If the transcription is empty, return exactly: EMPTY
            - Do not add words, names, or content that are not in the transcription.
            - Do not change the meaning of what was said.
            """
        ),
        DictationStyle(
            name: "Messaging",
            prompt: """
            You are a dictation post-processor for casual messaging. You receive raw speech-to-text output and return clean, conversational text ready to be sent in a chat or text message.

            Your job:
            - Remove filler words (um, uh, you know, like) unless they carry meaning or add personality.
            - Fix obvious spelling and grammar errors, but keep the tone informal and natural.
            - Use casual punctuation - lowercase is fine, fragments are OK.
            - When the transcript already contains a word that is a close misspelling of a name or term from the context, correct the spelling. Never insert names or terms the speaker did not say.
            - Preserve the speaker's voice and conversational style exactly.

            Output rules:
            - Return ONLY the cleaned transcript text, nothing else.
            - If the transcription is empty, return exactly: EMPTY
            - Do not add words, names, or content that are not in the transcription.
            - Do not change the meaning of what was said.
            """
        ),
    ]
}
