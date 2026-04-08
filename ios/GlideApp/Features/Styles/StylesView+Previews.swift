import SwiftUI

#Preview("Styles") {
    let settings = SettingsStore(preview: true)
    settings.sttProvider = "openai"
    settings.sttModel = "gpt-4o-mini-transcribe"
    settings.llmProvider = "openai"
    settings.llmModel = "gpt-4.1-mini"
    settings.systemPrompt = "You are a helpful dictation assistant."
    settings.styles = [
        DictationStyle(
            name: "Clinical",
            prompt: "Rewrite as a concise clinical note."
        ),
        DictationStyle(
            name: "Email",
            prompt: "Turn this into a polished email.",
            sttProvider: "openai",
            sttModel: "gpt-4o-mini-transcribe",
            llmProvider: "openai",
            llmModel: "gpt-4.1-mini"
        ),
    ]

    let liveSession = LiveDictationManager(preview: true)

    return StylesView()
        .environment(settings)
        .environment(liveSession)
}

#Preview("Style Edit") {
    struct PreviewHost: View {
        @State private var style = DictationStyle(
            name: "Clinical",
            prompt: "Rewrite as a concise clinical note."
        )

        var body: some View {
            let settings = SettingsStore(preview: true)
            settings.sttProvider = "openai"
            settings.sttModel = "gpt-4o-mini-transcribe"
            settings.llmProvider = "openai"
            settings.llmModel = "gpt-4.1-mini"

            return NavigationStack {
                StyleEditView(style: $style)
            }
            .environment(settings)
        }
    }

    return PreviewHost()
}

#Preview("Glide Wordmark") {
    GlideWordmark()
        .padding()
}
