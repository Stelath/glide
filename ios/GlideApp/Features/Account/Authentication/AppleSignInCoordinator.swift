import AuthenticationServices
import Foundation

enum AuthenticationError: LocalizedError, Sendable {
    case userCancelled
    case missingIdentityToken
    case invalidResponse(String)
    case providerNotConfigured(String)
    case stateMismatch
    case tokenExchangeFailed(String)
    case decodingFailed(String)

    var errorDescription: String? {
        switch self {
        case .userCancelled:
            return "Sign in was cancelled."
        case .missingIdentityToken:
            return "Sign in did not return an identity token."
        case .invalidResponse(let detail):
            return "Invalid response from provider: \(detail)"
        case .providerNotConfigured(let provider):
            return "\(provider) sign in is not configured for this build."
        case .stateMismatch:
            return "Sign in failed: state parameter did not match."
        case .tokenExchangeFailed(let detail):
            return "Token exchange failed: \(detail)"
        case .decodingFailed(let detail):
            return "Failed to decode ID token: \(detail)"
        }
    }
}

enum AppleSignInCoordinator {
    static func account(from authorization: ASAuthorization) throws -> (Account, AccountTokens) {
        guard let credential = authorization.credential as? ASAuthorizationAppleIDCredential else {
            throw AuthenticationError.invalidResponse("Unexpected credential type")
        }
        guard let identityTokenData = credential.identityToken,
              let identityToken = String(data: identityTokenData, encoding: .utf8) else {
            throw AuthenticationError.missingIdentityToken
        }

        let authCode = credential.authorizationCode.flatMap { String(data: $0, encoding: .utf8) }

        let fullName = [credential.fullName?.givenName, credential.fullName?.familyName]
            .compactMap { $0 }
            .filter { !$0.isEmpty }
            .joined(separator: " ")

        let account = Account(
            provider: .apple,
            stableId: credential.user,
            email: credential.email,
            displayName: fullName.isEmpty ? nil : fullName,
            avatarURL: nil,
            issuedAt: Date(),
            expiresAt: nil,
            subscriptionTier: .unsubscribed,
            tokenUsage: .zero
        )

        // Apple does not issue a refresh token at this layer; we stash the
        // authorization code in the accessToken slot so Phase 2 can trade it
        // for a server-side session.
        let tokens = AccountTokens(
            idToken: identityToken,
            accessToken: authCode,
            refreshToken: nil
        )
        return (account, tokens)
    }

    static func normalize(_ error: Error) -> Error {
        if let authError = error as? ASAuthorizationError, authError.code == .canceled {
            return AuthenticationError.userCancelled
        }
        return error
    }
}
