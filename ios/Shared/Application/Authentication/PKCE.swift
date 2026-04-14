import CryptoKit
import Foundation

/// PKCE (Proof Key for Code Exchange — RFC 7636) helpers.
///
/// The verifier is a cryptographically random URL-safe string; the challenge
/// is its base64url-encoded SHA256 digest. We always use S256, never plain.
enum PKCE {
    struct Pair: Sendable {
        let verifier: String
        let challenge: String
    }

    /// Generate a fresh verifier/challenge pair. `length` is the raw-byte count
    /// of the verifier source, not the base64url-encoded length.
    static func generate(length: Int = 64) -> Pair {
        let verifier = randomURLSafeString(byteCount: length)
        let challenge = sha256Challenge(verifier)
        return Pair(verifier: verifier, challenge: challenge)
    }

    /// Generate a random state parameter for CSRF protection in the OAuth flow.
    static func generateState(byteCount: Int = 32) -> String {
        randomURLSafeString(byteCount: byteCount)
    }

    // MARK: - Internals

    private static func randomURLSafeString(byteCount: Int) -> String {
        var bytes = [UInt8](repeating: 0, count: byteCount)
        let status = SecRandomCopyBytes(kSecRandomDefault, byteCount, &bytes)
        if status != errSecSuccess {
            for i in 0..<byteCount {
                bytes[i] = UInt8.random(in: 0...255)
            }
        }
        return Data(bytes).base64URLEncodedString()
    }

    private static func sha256Challenge(_ verifier: String) -> String {
        let digest = SHA256.hash(data: Data(verifier.utf8))
        return Data(digest).base64URLEncodedString()
    }
}

extension Data {
    /// Base64url (RFC 4648 §5) — standard base64 without padding, using `-` and `_`.
    func base64URLEncodedString() -> String {
        base64EncodedString()
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "=", with: "")
    }

    init?(base64URLEncoded string: String) {
        var s = string
            .replacingOccurrences(of: "-", with: "+")
            .replacingOccurrences(of: "_", with: "/")
        let padLength = (4 - s.count % 4) % 4
        s.append(String(repeating: "=", count: padLength))
        self.init(base64Encoded: s)
    }
}
