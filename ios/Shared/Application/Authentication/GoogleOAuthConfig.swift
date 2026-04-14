import Foundation

/// Google OAuth configuration for the iOS app.
///
/// This is a **public** OAuth client — Google's iOS client type does not use a
/// client secret. It is safe to commit the client ID here. Provision it in the
/// Google Cloud Console under "APIs & Services → Credentials → Create OAuth
/// client ID → iOS". Use the app's bundle identifier (`com.stelath.glide.app`).
///
/// The redirect URI is a reverse-DNS scheme derived from the client ID; register
/// it as a URL scheme under `CFBundleURLTypes` in `Info.plist`.
///
/// While the constant is empty the Google sign-in button is disabled at runtime
/// so the rest of the app (Apple SSO + Guest + BYO-API-key) still works.
enum GoogleOAuthConfig {
    /// Full OAuth 2.0 client ID from Google Cloud Console.
    /// Example format: `123456789012-abcdefghijklmnop.apps.googleusercontent.com`
    static let clientID: String = ""

    /// `openid email profile` gets us the `sub` claim plus display name and picture.
    static let scopes: [String] = ["openid", "email", "profile"]

    static let authorizationEndpoint = URL(string: "https://accounts.google.com/o/oauth2/v2/auth")!
    static let tokenEndpoint = URL(string: "https://oauth2.googleapis.com/token")!

    /// True if the client ID has been set — controls whether the Google button is enabled.
    static var isConfigured: Bool {
        !clientID.trimmingCharacters(in: .whitespaces).isEmpty
    }

    /// Reverse-DNS redirect URI scheme derived from the client ID.
    /// Apps register this under `CFBundleURLTypes`.
    static var redirectScheme: String? {
        guard isConfigured else { return nil }
        // Google iOS client IDs look like `<num>-<hash>.apps.googleusercontent.com`.
        // The redirect URL scheme is the same string reversed component-wise: `com.googleusercontent.apps.<num>-<hash>`.
        let components = clientID.split(separator: ".").reversed()
        return components.joined(separator: ".")
    }

    /// Full redirect URI: `<scheme>:/oauth2redirect`.
    static var redirectURI: String? {
        guard let scheme = redirectScheme else { return nil }
        return "\(scheme):/oauth2redirect"
    }
}
