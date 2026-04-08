import SwiftUI

struct ToggleAnimation: View {
    @Environment(SettingsStore.self) private var settings
    @State private var isOn = false

    var body: some View {
        VStack(spacing: 20) {
            HStack(spacing: 14) {
                Image(systemName: "keyboard")
                    .font(.body.weight(.medium))
                    .foregroundStyle(.white)
                    .frame(width: 30, height: 30)
                    .background(
                        RoundedRectangle(cornerRadius: 7)
                            .fill(settings.accent.primary)
                    )

                Text("Allow Full Access")
                    .font(.body)

                Spacer()

                Capsule()
                    .fill(isOn ? Color.green : settings.accent.accentSurface)
                    .frame(width: 51, height: 31)
                    .overlay(alignment: isOn ? .trailing : .leading) {
                        Circle()
                            .fill(.white)
                            .shadow(color: .black.opacity(0.15), radius: 2, y: 1)
                            .frame(width: 27, height: 27)
                            .padding(2)
                    }
                    .animation(.spring(duration: 0.35, bounce: 0.15), value: isOn)
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 12)
            .background(
                RoundedRectangle(cornerRadius: 14)
                    .fill(Color.glideSurface)
            )
            .padding(.horizontal, 8)

            if !isOn {
                Image(systemName: "hand.tap.fill")
                    .font(.title2)
                    .foregroundStyle(.secondary)
                    .symbolEffect(.pulse, options: .repeating)
            } else {
                Image(systemName: "checkmark.circle.fill")
                    .font(.title2)
                    .foregroundStyle(.green)
                    .transition(.scale.combined(with: .opacity))
            }
        }
        .onAppear {
            startAnimation()
        }
    }

    private func startAnimation() {
        func cycle() {
            DispatchQueue.main.asyncAfter(deadline: .now() + 1.2) {
                withAnimation {
                    isOn = true
                }
            }
            DispatchQueue.main.asyncAfter(deadline: .now() + 3.5) {
                withAnimation {
                    isOn = false
                }
            }
            DispatchQueue.main.asyncAfter(deadline: .now() + 4.5) {
                cycle()
            }
        }

        cycle()
    }
}
