import AuthenticationServices
import SwiftUI

struct WelcomeSignInView: View {
    @Environment(SettingsStore.self) private var settings
    @Environment(AccountStore.self) private var accountStore
    @Environment(\.colorScheme) private var colorScheme

    @State private var errorMessage: String?
    @State private var isAuthenticating = false
    @State private var appeared = false

    var body: some View {
        ZStack {
            LinearGradient(
                colors: [Color.glideBackground, Color.glideSurface],
                startPoint: .top,
                endPoint: .bottom
            )
            .ignoresSafeArea()

            VStack(spacing: 32) {
                Spacer()

                header

                Spacer()

                signInButtons
                    .padding(.horizontal, 32)

                guestButton
                    .padding(.bottom, 28)
            }
            .opacity(appeared ? 1 : 0)
            .offset(y: appeared ? 0 : 16)
        }
        .onAppear {
            withAnimation(.easeOut(duration: 0.5).delay(0.15)) {
                appeared = true
            }
        }
        .alert("Sign In Failed", isPresented: errorBinding) {
            Button("OK", role: .cancel) { errorMessage = nil }
        } message: {
            Text(errorMessage ?? "")
        }
    }

    private var header: some View {
        VStack(spacing: 16) {
            GlideWordmark()
                .frame(height: 40)

            VStack(spacing: 8) {
                Text("Welcome to Glide")
                    .font(.largeTitle.weight(.bold))
                    .foregroundStyle(Color.glideText)

                Text("Sign in to sync your account and unlock a plan\nwithout managing your own API keys.")
                    .font(.body)
                    .foregroundStyle(.secondary)
                    .multilineTextAlignment(.center)
            }
        }
    }

    private var signInButtons: some View {
        VStack(spacing: 12) {
            SignInWithAppleButton(.continue) { request in
                request.requestedScopes = [.fullName, .email]
            } onCompletion: { result in
                handleAppleResult(result)
            }
            .signInWithAppleButtonStyle(colorScheme == .dark ? .white : .black)
            .frame(height: 52)
            .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
            .disabled(isAuthenticating)
            .accessibilityLabel("Sign in with Apple")

            Button {
                Task { await runGoogleSignIn() }
            } label: {
                HStack(spacing: 10) {
                    Image(systemName: "g.circle.fill")
                        .font(.title2)
                    Text("Continue with Google")
                        .font(.body.weight(.semibold))
                }
                .frame(maxWidth: .infinity)
                .frame(height: 52)
                .foregroundStyle(Color.glideText)
                .background(
                    RoundedRectangle(cornerRadius: 12, style: .continuous)
                        .fill(Color.glideSurface)
                )
                .overlay(
                    RoundedRectangle(cornerRadius: 12, style: .continuous)
                        .strokeBorder(Color.glideText.opacity(0.15), lineWidth: 1)
                )
            }
            .disabled(isAuthenticating || !GoogleOAuthConfig.isConfigured)
            .opacity(GoogleOAuthConfig.isConfigured ? 1 : 0.5)
            .accessibilityLabel("Continue with Google")

            if !GoogleOAuthConfig.isConfigured {
                Text("Google sign in is not configured in this build.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        }
    }

    private var guestButton: some View {
        Button {
            accountStore.continueAsGuest()
        } label: {
            Text("Continue as Guest")
                .font(.subheadline.weight(.medium))
                .foregroundStyle(Color.glideText.opacity(0.6))
                .underline()
        }
        .buttonStyle(.plain)
        .disabled(isAuthenticating)
    }

    private func handleAppleResult(_ result: Result<ASAuthorization, Error>) {
        switch result {
        case .success(let authorization):
            do {
                let (account, tokens) = try AppleSignInCoordinator.account(from: authorization)
                accountStore.signIn(account: account, tokens: tokens)
            } catch {
                errorMessage = error.localizedDescription
            }
        case .failure(let error):
            let normalized = AppleSignInCoordinator.normalize(error)
            if case AuthenticationError.userCancelled = normalized { return }
            errorMessage = normalized.localizedDescription
        }
    }

    private func runGoogleSignIn() async {
        guard !isAuthenticating else { return }
        isAuthenticating = true
        defer { isAuthenticating = false }
        do {
            let coordinator = GoogleSignInCoordinator()
            let (account, tokens) = try await coordinator.signIn()
            accountStore.signIn(account: account, tokens: tokens)
        } catch AuthenticationError.userCancelled {
            return
        } catch {
            errorMessage = error.localizedDescription
        }
    }

    private var errorBinding: Binding<Bool> {
        Binding(
            get: { errorMessage != nil },
            set: { if !$0 { errorMessage = nil } }
        )
    }
}
