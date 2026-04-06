import SwiftUI

struct ProvidersView: View {
    @Environment(SettingsStore.self) private var settings

    @State private var showOpenAIKey = false
    @State private var showGroqKey = false
    @State private var showOpenAIAdvanced = false
    @State private var showGroqAdvanced = false

    var body: some View {
        @Bindable var settings = settings

        NavigationStack {
            VStack(spacing: 0) {
                HStack {
                    Text("Providers")
                        .font(.subheadline.weight(.semibold))
                        .foregroundStyle(Color.glideText)
                }
                .frame(maxWidth: .infinity)
                .padding(.vertical, 10)
                .background(Color(.systemBackground))

                Divider()

                ScrollView {
                    VStack(spacing: 16) {
                        ProviderCard(
                            provider: .openAI,
                            apiKey: $settings.openAIApiKey,
                            baseURL: $settings.openAIBaseURL,
                            showKey: $showOpenAIKey,
                            showAdvanced: $showOpenAIAdvanced,
                            verification: settings.openAIVerification
                        )

                        ProviderCard(
                            provider: .groq,
                            apiKey: $settings.groqApiKey,
                            baseURL: $settings.groqBaseURL,
                            showKey: $showGroqKey,
                            showAdvanced: $showGroqAdvanced,
                            verification: settings.groqVerification
                        )
                    }
                    .padding(.horizontal, 16)
                    .padding(.top, 8)
                }
                .background(Color(.systemGroupedBackground))
            }
            .toolbar(.hidden, for: .navigationBar)
            .onAppear {
                if !settings.openAIApiKey.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty && !settings.openAIVerified {
                    settings.scheduleAutoVerify(for: ProviderInfo.openAI.id)
                }
                if !settings.groqApiKey.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty && !settings.groqVerified {
                    settings.scheduleAutoVerify(for: ProviderInfo.groq.id)
                }
            }
            .onChange(of: settings.openAIApiKey) { _, _ in
                settings.scheduleAutoVerify(for: ProviderInfo.openAI.id)
            }
            .onChange(of: settings.openAIBaseURL) { _, _ in
                settings.scheduleAutoVerify(for: ProviderInfo.openAI.id)
            }
            .onChange(of: settings.groqApiKey) { _, _ in
                settings.scheduleAutoVerify(for: ProviderInfo.groq.id)
            }
            .onChange(of: settings.groqBaseURL) { _, _ in
                settings.scheduleAutoVerify(for: ProviderInfo.groq.id)
            }
        }
    }
}

// MARK: - Provider Card

private struct ProviderCard: View {
    let provider: ProviderInfo
    @Binding var apiKey: String
    @Binding var baseURL: String
    @Binding var showKey: Bool
    @Binding var showAdvanced: Bool
    let verification: VerificationState

    var body: some View {
        VStack(spacing: 0) {
            // Header
            HStack(spacing: 12) {
                Image(provider.logoAssetName)
                    .resizable()
                    .scaledToFill()
                    .frame(width: 36, height: 36)
                    .clipShape(RoundedRectangle(cornerRadius: 8))

                VStack(alignment: .leading, spacing: 2) {
                    Text(provider.displayName)
                        .font(.headline)

                    statusText
                }

                Spacer()

                statusIcon
            }
            .padding(16)

            Divider()
                .padding(.leading, 16)

            // API Key field
            VStack(spacing: 12) {
                HStack(spacing: 12) {
                    Group {
                        if showKey {
                            TextField("API Key", text: $apiKey)
                                .textInputAutocapitalization(.never)
                                .autocorrectionDisabled()
                                .textContentType(.password)
                        } else {
                            SecureField("API Key", text: $apiKey)
                                .textContentType(.password)
                        }
                    }
                    .font(.body)

                    Button {
                        showKey.toggle()
                    } label: {
                        Image(systemName: showKey ? "eye.slash" : "eye")
                            .foregroundStyle(.secondary)
                            .frame(width: 28, height: 28)
                    }
                    .buttonStyle(.plain)
                }

                // Error message
                if case .failed(let message) = verification {
                    HStack(spacing: 6) {
                        Image(systemName: "exclamationmark.triangle.fill")
                            .font(.caption)
                        Text(message)
                            .font(.caption)
                    }
                    .foregroundStyle(.red)
                    .frame(maxWidth: .infinity, alignment: .leading)
                }

                // Advanced (Base URL)
                if showAdvanced {
                    VStack(alignment: .leading, spacing: 6) {
                        Text("Base URL")
                            .font(.caption)
                            .foregroundStyle(.secondary)

                        TextField("Base URL", text: $baseURL)
                            .textInputAutocapitalization(.never)
                            .autocorrectionDisabled()
                            .textContentType(.URL)
                            .font(.body)
                    }
                }

                Button {
                    withAnimation(.easeInOut(duration: 0.2)) {
                        showAdvanced.toggle()
                    }
                } label: {
                    HStack(spacing: 4) {
                        Text(showAdvanced ? "Hide Advanced" : "Advanced")
                            .font(.caption)
                        Image(systemName: "chevron.right")
                            .font(.caption2)
                            .rotationEffect(.degrees(showAdvanced ? 90 : 0))
                    }
                    .foregroundStyle(.secondary)
                }
                .buttonStyle(.plain)
                .frame(maxWidth: .infinity, alignment: .leading)
            }
            .padding(16)
        }
        .background(Color(.secondarySystemGroupedBackground))
        .clipShape(RoundedRectangle(cornerRadius: 14))
        .overlay(alignment: .leading) {
            // Accent strip based on verification status
            accentStrip
        }
        .clipShape(RoundedRectangle(cornerRadius: 14))
    }

    // MARK: - Status Elements

    private var providerColor: Color {
        provider.id == ProviderInfo.openAI.id ? .blue : .orange
    }

    @ViewBuilder
    private var statusText: some View {
        switch verification {
        case .idle:
            Text("Not connected")
                .font(.caption)
                .foregroundStyle(.secondary)
        case .verifying:
            Text("Verifying...")
                .font(.caption)
                .foregroundStyle(.secondary)
        case .verified:
            Text("Connected")
                .font(.caption)
                .foregroundStyle(.green)
        case .failed:
            Text("Connection failed")
                .font(.caption)
                .foregroundStyle(.red)
        }
    }

    @ViewBuilder
    private var statusIcon: some View {
        switch verification {
        case .idle:
            EmptyView()
        case .verifying:
            ProgressView()
                .controlSize(.small)
        case .verified:
            Image(systemName: "checkmark.circle.fill")
                .font(.title3)
                .foregroundStyle(.green)
        case .failed:
            Image(systemName: "xmark.circle.fill")
                .font(.title3)
                .foregroundStyle(.red)
        }
    }

    @ViewBuilder
    private var accentStrip: some View {
        switch verification {
        case .verified:
            UnevenRoundedRectangle(topLeadingRadius: 14, bottomLeadingRadius: 14)
                .fill(Color.green)
                .frame(width: 4)
        case .failed:
            UnevenRoundedRectangle(topLeadingRadius: 14, bottomLeadingRadius: 14)
                .fill(Color.red)
                .frame(width: 4)
        case .verifying:
            UnevenRoundedRectangle(topLeadingRadius: 14, bottomLeadingRadius: 14)
                .fill(Color.glidePrimary.opacity(0.5))
                .frame(width: 4)
        case .idle:
            EmptyView()
        }
    }
}
