import SwiftUI

struct AccountChip: View {
    @Environment(AccountStore.self) private var accountStore
    @Environment(SettingsStore.self) private var settings

    @State private var showingAccountSheet = false

    var body: some View {
        Button {
            showingAccountSheet = true
        } label: {
            HStack(spacing: 8) {
                avatar
                Text(label)
                    .font(.subheadline.weight(.semibold))
                    .foregroundStyle(Color.glideText)
                    .lineLimit(1)
                    .truncationMode(.tail)
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 8)
            .frame(maxWidth: 180)
            .background(
                Capsule(style: .continuous)
                    .fill(Color.glideSurface)
                    .shadow(color: .black.opacity(0.08), radius: 6, x: 0, y: 2)
            )
            .overlay(
                Capsule(style: .continuous)
                    .strokeBorder(Color.glideText.opacity(0.08), lineWidth: 1)
            )
        }
        .buttonStyle(.plain)
        .padding(.trailing, 16)
        .padding(.bottom, 12)
        .accessibilityLabel(accessibilityLabel)
        .sheet(isPresented: $showingAccountSheet) {
            AccountView()
                .presentationDetents([.medium, .large])
                .presentationDragIndicator(.visible)
        }
    }

    @ViewBuilder
    private var avatar: some View {
        if let account = accountStore.currentAccount {
            if let url = account.avatarURL {
                AsyncImage(url: url) { image in
                    image.resizable().scaledToFill()
                } placeholder: {
                    AccountAvatarView(initials: account.initials, accent: settings.accent.primary, size: 24)
                }
                .frame(width: 24, height: 24)
                .clipShape(Circle())
            } else {
                AccountAvatarView(initials: account.initials, accent: settings.accent.primary, size: 24)
            }
        } else {
            Image(systemName: "person.crop.circle")
                .font(.title3)
                .foregroundStyle(Color.glideText.opacity(0.5))
                .frame(width: 24, height: 24)
        }
    }

    private var label: String {
        accountStore.currentAccount?.shortDisplayName ?? "Guest"
    }

    private var accessibilityLabel: String {
        if let account = accountStore.currentAccount {
            return "Account: \(account.shortDisplayName)"
        }
        return "Account: Guest. Tap to sign in."
    }
}
