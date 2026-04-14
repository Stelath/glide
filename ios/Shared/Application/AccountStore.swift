import Foundation
import Observation

@MainActor
@Observable
final class AccountStore {
    static let shared = AccountStore()

    static let suiteName = SettingsStore.suiteName
    nonisolated static let notificationName = "com.glide.account-changed"

    @ObservationIgnored
    private let store: AppGroupKeyValueStore

    @ObservationIgnored
    private let secretStore: KeychainSecretStore

    @ObservationIgnored
    private var isReloading = false

    /// Signed-in account (nil = guest).
    var currentAccount: Account? {
        didSet { persistAccount() }
    }

    /// Set to true when the user has seen the welcome screen, whether they signed in or chose Guest.
    /// Used to gate the first-launch welcome view.
    var hasSeenWelcome: Bool {
        didSet { persistBool(hasSeenWelcome, key: Self.hasSeenWelcomeKey) }
    }

    // MARK: - Init

    private init() {
        store = AppGroupKeyValueStore(appGroupIdentifier: Self.suiteName, fileName: "account.store")
        secretStore = KeychainSecretStore(accessGroup: Self.suiteName, fallbackStore: store)

        currentAccount = nil
        hasSeenWelcome = false

        reloadFromDisk()
    }

    init(preview: Bool, account: Account? = nil, hasSeenWelcome: Bool = true) {
        store = AppGroupKeyValueStore(appGroupIdentifier: Self.suiteName, fileName: "account.store")
        secretStore = KeychainSecretStore(accessGroup: Self.suiteName, fallbackStore: store)
        currentAccount = account
        self.hasSeenWelcome = hasSeenWelcome
    }

    // MARK: - Public

    var isSignedIn: Bool { currentAccount != nil }

    /// Persist a freshly signed-in account along with its tokens.
    /// Called by the Apple and Google sign-in coordinators on success.
    func signIn(account: Account, tokens: AccountTokens) {
        writeTokens(tokens, for: account)
        currentAccount = account
        hasSeenWelcome = true
    }

    /// User chose "Continue as Guest" from the welcome screen. Leaves currentAccount nil
    /// but records the welcome-seen flag so we don't show the welcome view again.
    func continueAsGuest() {
        hasSeenWelcome = true
        postAccountChangedNotification()
    }

    /// Clear account + all stored tokens. Does NOT reset hasSeenWelcome.
    func signOut() {
        if let account = currentAccount {
            clearTokens(for: account)
        }
        currentAccount = nil
    }

    /// Load tokens for the current account from Keychain. Returns nil if not signed in.
    /// Phase 2 will call this when routing through the Glide backend.
    func loadCurrentTokens() -> AccountTokens? {
        guard let account = currentAccount else { return nil }
        return readTokens(for: account)
    }

    // MARK: - Reload

    func reloadFromDisk() {
        isReloading = true
        defer { isReloading = false }

        hasSeenWelcome = store.bool(forKey: Self.hasSeenWelcomeKey)
        if let data = store.data(forKey: Self.currentAccountKey),
           let decoded = try? JSONDecoder.accountDecoder.decode(Account.self, from: data) {
            currentAccount = decoded
        } else {
            currentAccount = nil
        }
    }

    // MARK: - Persistence

    private func persistAccount() {
        guard !isReloading else { return }
        if let account = currentAccount,
           let data = try? JSONEncoder.accountEncoder.encode(account) {
            store.set(data, forKey: Self.currentAccountKey)
        } else {
            store.removeObject(forKey: Self.currentAccountKey)
        }
        postAccountChangedNotification()
    }

    private func persistBool(_ value: Bool, key: String) {
        guard !isReloading else { return }
        store.set(value, forKey: key)
    }

    // MARK: - Token storage (Keychain)

    private func writeTokens(_ tokens: AccountTokens, for account: Account) {
        secretStore.write(service: Self.idTokenService, account: account.id, value: tokens.idToken)
        secretStore.write(service: Self.refreshTokenService, account: account.id, value: tokens.refreshToken ?? "")
        secretStore.write(service: Self.accessTokenService, account: account.id, value: tokens.accessToken ?? "")
    }

    private func readTokens(for account: Account) -> AccountTokens? {
        let idToken = secretStore.read(service: Self.idTokenService, account: account.id)
        guard !idToken.isEmpty else { return nil }
        let refresh = secretStore.read(service: Self.refreshTokenService, account: account.id)
        let access = secretStore.read(service: Self.accessTokenService, account: account.id)
        return AccountTokens(
            idToken: idToken,
            accessToken: access.isEmpty ? nil : access,
            refreshToken: refresh.isEmpty ? nil : refresh
        )
    }

    private func clearTokens(for account: Account) {
        secretStore.delete(service: Self.idTokenService, account: account.id)
        secretStore.delete(service: Self.refreshTokenService, account: account.id)
        secretStore.delete(service: Self.accessTokenService, account: account.id)
    }

    // MARK: - Notifications

    private func postAccountChangedNotification() {
        let center = CFNotificationCenterGetDarwinNotifyCenter()
        let name = CFNotificationName(Self.notificationName as CFString)
        CFNotificationCenterPostNotification(center, name, nil, nil, true)
    }

    // MARK: - Keys

    private static let currentAccountKey = "current_account"
    private static let hasSeenWelcomeKey = "has_seen_welcome"

    private static let idTokenService = "glide-account-idtoken"
    private static let accessTokenService = "glide-account-accesstoken"
    private static let refreshTokenService = "glide-account-refreshtoken"
}

/// OAuth/OIDC tokens returned by a sign-in flow. Never persisted to plist —
/// only held in memory and written to Keychain via AccountStore.
struct AccountTokens: Sendable, Equatable {
    var idToken: String
    var accessToken: String?
    var refreshToken: String?
}

private extension JSONEncoder {
    static let accountEncoder: JSONEncoder = {
        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .secondsSince1970
        return encoder
    }()
}

private extension JSONDecoder {
    static let accountDecoder: JSONDecoder = {
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .secondsSince1970
        return decoder
    }()
}
