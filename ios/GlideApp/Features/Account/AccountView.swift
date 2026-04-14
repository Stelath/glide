import AuthenticationServices
import SwiftUI

struct AccountView: View {
    @Environment(SettingsStore.self) private var settings
    @Environment(AccountStore.self) private var accountStore
    @Environment(\.dismiss) private var dismiss
    @Environment(\.colorScheme) private var colorScheme

    @State private var errorMessage: String?
    @State private var showingSignOutConfirmation = false
    @State private var isAuthenticating = false

    var body: some View {
        NavigationStack {
            Group {
                if let account = accountStore.currentAccount {
                    signedInView(account: account)
                } else {
                    guestView
                }
            }
            .background(Color.glideBackground)
            .navigationTitle("Account")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Button("Done") { dismiss() }
                }
            }
        }
        .alert("Sign In Failed", isPresented: errorBinding) {
            Button("OK", role: .cancel) { errorMessage = nil }
        } message: {
            Text(errorMessage ?? "")
        }
        .confirmationDialog(
            "Sign out of Glide?",
            isPresented: $showingSignOutConfirmation,
            titleVisibility: .visible
        ) {
            Button("Sign Out", role: .destructive) {
                accountStore.signOut()
            }
            Button("Cancel", role: .cancel) {}
        } message: {
            Text("You can sign back in any time.")
        }
    }

    private func signedInView(account: Account) -> some View {
        Form {
            Section {
                HStack(spacing: 14) {
                    avatar(for: account)

                    VStack(alignment: .leading, spacing: 2) {
                        Text(account.displayName ?? account.email ?? "Glide Account")
                            .font(.headline)
                            .foregroundStyle(Color.glideText)
                        if let email = account.email, account.displayName != nil {
                            Text(email)
                                .font(.subheadline)
                                .foregroundStyle(.secondary)
                        }
                        providerBadge(account.provider)
                            .padding(.top, 2)
                    }
                    Spacer()
                }
                .padding(.vertical, 4)
            }

            Section {
                Button(role: .destructive) {
                    showingSignOutConfirmation = true
                } label: {
                    Text("Sign Out")
                        .frame(maxWidth: .infinity, alignment: .center)
                }
            }
        }
        .scrollContentBackground(.hidden)
    }

    private var guestView: some View {
        Form {
            Section {
                VStack(spacing: 12) {
                    Image(systemName: "person.crop.circle.badge.questionmark")
                        .font(.system(size: 48, weight: .regular))
                        .foregroundStyle(settings.accent.primary)
                        .symbolRenderingMode(.hierarchical)
                    Text("You're using Glide as a guest.")
                        .font(.headline)
                        .foregroundStyle(Color.glideText)
                    Text("Sign in to unlock a plan without managing your own API keys (coming soon).")
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                        .multilineTextAlignment(.center)
                }
                .frame(maxWidth: .infinity)
                .padding(.vertical, 8)
            }

            Section {
                SignInWithAppleButton(.signIn) { request in
                    request.requestedScopes = [.fullName, .email]
                } onCompletion: { result in
                    handleAppleResult(result)
                }
                .signInWithAppleButtonStyle(colorScheme == .dark ? .white : .black)
                .frame(height: 48)
                .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
                .disabled(isAuthenticating)
                .listRowInsets(EdgeInsets(top: 6, leading: 16, bottom: 6, trailing: 16))

                Button {
                    Task { await runGoogleSignIn() }
                } label: {
                    HStack(spacing: 10) {
                        Image(systemName: "g.circle.fill")
                            .font(.title3)
                        Text("Continue with Google")
                            .font(.body.weight(.semibold))
                    }
                    .frame(maxWidth: .infinity)
                    .frame(height: 48)
                    .foregroundStyle(Color.glideText)
                    .background(
                        RoundedRectangle(cornerRadius: 10, style: .continuous)
                            .fill(Color.glideSurface)
                    )
                    .overlay(
                        RoundedRectangle(cornerRadius: 10, style: .continuous)
                            .strokeBorder(Color.glideText.opacity(0.15), lineWidth: 1)
                    )
                }
                .buttonStyle(.plain)
                .disabled(isAuthenticating || !GoogleOAuthConfig.isConfigured)
                .opacity(GoogleOAuthConfig.isConfigured ? 1 : 0.5)
                .listRowInsets(EdgeInsets(top: 6, leading: 16, bottom: 6, trailing: 16))

                if !GoogleOAuthConfig.isConfigured {
                    Text("Google sign in is not configured in this build.")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }
        }
        .scrollContentBackground(.hidden)
    }

    @ViewBuilder
    private func avatar(for account: Account) -> some View {
        if let url = account.avatarURL {
            AsyncImage(url: url) { image in
                image.resizable().scaledToFill()
            } placeholder: {
                AccountAvatarView(initials: account.initials, accent: settings.accent.primary, size: 56)
            }
            .frame(width: 56, height: 56)
            .clipShape(Circle())
        } else {
            AccountAvatarView(initials: account.initials, accent: settings.accent.primary, size: 56)
        }
    }

    private func providerBadge(_ provider: AuthProvider) -> some View {
        HStack(spacing: 4) {
            Image(systemName: provider == .apple ? "applelogo" : "g.circle.fill")
                .font(.caption2)
            Text("Signed in with \(provider.displayName)")
                .font(.caption2.weight(.medium))
        }
        .foregroundStyle(.secondary)
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
