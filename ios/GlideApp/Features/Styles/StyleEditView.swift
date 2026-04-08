import SwiftUI

struct StyleEditView: View {
    @Binding var style: DictationStyle
    @Environment(SettingsStore.self) private var settings

    var body: some View {
        Form {
            TextField("Name", text: $style.name)

            NavigationLink("Prompt") {
                PromptEditorView(title: style.name.isEmpty ? "Prompt" : style.name, text: $style.prompt)
            }

            Section("Model Overrides") {
                Picker("Voice Model", selection: sttOverrideBinding) {
                    Text("Default").tag(Optional<ModelOption>.none)
                    ForEach(settings.allSTTModels, id: \.self) { option in
                        Text(option.displayName).tag(Optional(option))
                    }
                }

                Picker("LLM Model", selection: llmOverrideBinding) {
                    Text(defaultLLMLabel).tag(Optional<ModelOption>.none)
                    ForEach(settings.allLLMModels, id: \.self) { option in
                        Text(option.displayName).tag(Optional(option))
                    }
                }
            }
        }
        .navigationTitle(style.name.isEmpty ? "Edit Style" : style.name)
    }

    private var defaultLLMLabel: String {
        "Default"
    }

    private var sttOverrideBinding: Binding<ModelOption?> {
        Binding(
            get: {
                guard let provider = style.sttProvider, let model = style.sttModel else { return nil }
                return ModelOption(provider: provider, model: model)
            },
            set: { newValue in
                style.sttProvider = newValue?.provider
                style.sttModel = newValue?.model
            }
        )
    }

    private var llmOverrideBinding: Binding<ModelOption?> {
        Binding(
            get: {
                guard let provider = style.llmProvider, let model = style.llmModel else { return nil }
                return ModelOption(provider: provider, model: model)
            },
            set: { newValue in
                style.llmProvider = newValue?.provider
                style.llmModel = newValue?.model
            }
        )
    }
}
