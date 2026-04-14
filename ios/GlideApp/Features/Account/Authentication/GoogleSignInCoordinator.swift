import AuthenticationServices
import Foundation
import UIKit

@MainActor
final class GoogleSignInCoordinator: NSObject {
    private var session: ASWebAuthenticationSession?

    func signIn() async throws -> (Account, AccountTokens) {
        guard GoogleOAuthConfig.isConfigured,
              let redirectURI = GoogleOAuthConfig.redirectURI,
              let redirectScheme = GoogleOAuthConfig.redirectScheme else {
            throw AuthenticationError.providerNotConfigured("Google")
        }

        let pkce = PKCE.generate()
        let expectedState = PKCE.generateState()

        let authorizeURL = try buildAuthorizeURL(redirectURI: redirectURI, challenge: pkce.challenge, state: expectedState)
        let callbackURL = try await presentAuthSession(url: authorizeURL, callbackScheme: redirectScheme)

        let (code, returnedState) = try parseCallback(callbackURL)
        guard returnedState == expectedState else {
            throw AuthenticationError.stateMismatch
        }

        let tokenResponse = try await exchangeCode(code: code, verifier: pkce.verifier, redirectURI: redirectURI)
        let claims = try decodeIDToken(tokenResponse.idToken)

        let now = Date()
        let expiresAt = tokenResponse.expiresIn.map { now.addingTimeInterval(TimeInterval($0)) }
        let account = Account(
            provider: .google,
            stableId: claims.sub,
            email: claims.email,
            displayName: claims.name,
            avatarURL: claims.picture.flatMap(URL.init(string:)),
            issuedAt: now,
            expiresAt: expiresAt,
            subscriptionTier: .unsubscribed,
            tokenUsage: .zero
        )
        let tokens = AccountTokens(
            idToken: tokenResponse.idToken,
            accessToken: tokenResponse.accessToken,
            refreshToken: tokenResponse.refreshToken
        )
        return (account, tokens)
    }

    private func buildAuthorizeURL(redirectURI: String, challenge: String, state: String) throws -> URL {
        var components = URLComponents(url: GoogleOAuthConfig.authorizationEndpoint, resolvingAgainstBaseURL: false)
        components?.queryItems = [
            URLQueryItem(name: "client_id", value: GoogleOAuthConfig.clientID),
            URLQueryItem(name: "redirect_uri", value: redirectURI),
            URLQueryItem(name: "response_type", value: "code"),
            URLQueryItem(name: "scope", value: GoogleOAuthConfig.scopes.joined(separator: " ")),
            URLQueryItem(name: "code_challenge", value: challenge),
            URLQueryItem(name: "code_challenge_method", value: "S256"),
            URLQueryItem(name: "state", value: state),
            URLQueryItem(name: "prompt", value: "select_account"),
        ]
        guard let url = components?.url else {
            throw AuthenticationError.invalidResponse("Could not build authorize URL")
        }
        return url
    }

    private func presentAuthSession(url: URL, callbackScheme: String) async throws -> URL {
        try await withCheckedThrowingContinuation { continuation in
            let session = ASWebAuthenticationSession(
                url: url,
                callbackURLScheme: callbackScheme
            ) { callbackURL, error in
                if let error {
                    if let authError = error as? ASWebAuthenticationSessionError,
                       authError.code == .canceledLogin {
                        continuation.resume(throwing: AuthenticationError.userCancelled)
                    } else {
                        continuation.resume(throwing: error)
                    }
                    return
                }
                guard let callbackURL else {
                    continuation.resume(throwing: AuthenticationError.invalidResponse("Empty callback URL"))
                    return
                }
                continuation.resume(returning: callbackURL)
            }
            session.presentationContextProvider = self
            session.prefersEphemeralWebBrowserSession = true
            self.session = session
            session.start()
        }
    }

    private func parseCallback(_ url: URL) throws -> (code: String, state: String) {
        guard let components = URLComponents(url: url, resolvingAgainstBaseURL: false) else {
            throw AuthenticationError.invalidResponse("Bad callback URL")
        }
        if let errorItem = components.queryItems?.first(where: { $0.name == "error" })?.value {
            throw AuthenticationError.invalidResponse("OAuth error: \(errorItem)")
        }
        guard let code = components.queryItems?.first(where: { $0.name == "code" })?.value, !code.isEmpty else {
            throw AuthenticationError.invalidResponse("Missing authorization code")
        }
        guard let state = components.queryItems?.first(where: { $0.name == "state" })?.value else {
            throw AuthenticationError.stateMismatch
        }
        return (code, state)
    }

    private func exchangeCode(code: String, verifier: String, redirectURI: String) async throws -> GoogleTokenResponse {
        var request = URLRequest(url: GoogleOAuthConfig.tokenEndpoint)
        request.httpMethod = "POST"
        request.setValue("application/x-www-form-urlencoded", forHTTPHeaderField: "Content-Type")

        var bodyComponents = URLComponents()
        bodyComponents.queryItems = [
            URLQueryItem(name: "client_id", value: GoogleOAuthConfig.clientID),
            URLQueryItem(name: "code", value: code),
            URLQueryItem(name: "code_verifier", value: verifier),
            URLQueryItem(name: "grant_type", value: "authorization_code"),
            URLQueryItem(name: "redirect_uri", value: redirectURI),
        ]
        request.httpBody = bodyComponents.percentEncodedQuery?.data(using: .utf8)

        let (data, response) = try await URLSession.shared.data(for: request)
        guard let http = response as? HTTPURLResponse, (200...299).contains(http.statusCode) else {
            let detail = String(data: data, encoding: .utf8) ?? "(no body)"
            throw AuthenticationError.tokenExchangeFailed(detail)
        }
        do {
            return try JSONDecoder().decode(GoogleTokenResponse.self, from: data)
        } catch {
            throw AuthenticationError.tokenExchangeFailed("Decode error: \(error.localizedDescription)")
        }
    }

    private func decodeIDToken(_ token: String) throws -> IDTokenClaims {
        let segments = token.split(separator: ".")
        guard segments.count >= 2 else {
            throw AuthenticationError.decodingFailed("JWT had fewer than 2 segments")
        }
        guard let payloadData = Data(base64URLEncoded: String(segments[1])) else {
            throw AuthenticationError.decodingFailed("Could not base64url-decode payload")
        }
        do {
            return try JSONDecoder().decode(IDTokenClaims.self, from: payloadData)
        } catch {
            throw AuthenticationError.decodingFailed(error.localizedDescription)
        }
    }
}

extension GoogleSignInCoordinator: ASWebAuthenticationPresentationContextProviding {
    func presentationAnchor(for session: ASWebAuthenticationSession) -> ASPresentationAnchor {
        let scenes = UIApplication.shared.connectedScenes
            .compactMap { $0 as? UIWindowScene }
            .first(where: { $0.activationState == .foregroundActive })
            ?? UIApplication.shared.connectedScenes.compactMap({ $0 as? UIWindowScene }).first
        return scenes?.windows.first(where: { $0.isKeyWindow })
            ?? scenes?.windows.first
            ?? ASPresentationAnchor()
    }
}

private struct GoogleTokenResponse: Decodable, Sendable {
    let accessToken: String?
    let idToken: String
    let refreshToken: String?
    let expiresIn: Int?

    enum CodingKeys: String, CodingKey {
        case accessToken = "access_token"
        case idToken = "id_token"
        case refreshToken = "refresh_token"
        case expiresIn = "expires_in"
    }
}

private struct IDTokenClaims: Decodable, Sendable {
    let sub: String
    let email: String?
    let name: String?
    let picture: String?
}
