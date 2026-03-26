import SwiftUI

struct ContentView: View {
    var body: some View {
        VStack(spacing: 20) {
            Image(systemName: "mic.circle.fill")
                .font(.system(size: 80))
                .foregroundStyle(.blue)

            Text("Glide")
                .font(.largeTitle.bold())

            Text("Enable the Glide keyboard in Settings \u{2192} General \u{2192} Keyboard \u{2192} Keyboards to dictate in any app.")
                .font(.body)
                .multilineTextAlignment(.center)
                .padding(.horizontal)

            Text("Core version: \(coreVersion)")
                .font(.caption)
                .foregroundStyle(.secondary)
        }
        .padding()
    }

    private var coreVersion: String {
        String(cString: glide_core_version())
    }
}
