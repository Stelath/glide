import SwiftUI

// MARK: - Styles Tab

struct StylesView: View {
    @Environment(SettingsStore.self) private var settings
    @Environment(LiveDictationManager.self) private var liveSession

    var body: some View {
        @Bindable var settings = settings

        NavigationStack {
            VStack(spacing: 0) {
                // Custom top bar
                HStack {
                    Spacer()

                    MiniWaveform(compact: true)
                        .frame(height: 24)

                    Spacer()

                    Image(systemName: "mic.fill")
                        .font(.subheadline.weight(.medium))
                        .foregroundStyle(liveSession.snapshot.phase.isActive ? .white : .secondary)
                        .frame(width: 34, height: 34)
                        .background(
                            RoundedRectangle(cornerRadius: 8)
                                .fill(liveSession.snapshot.phase.isActive ? Color.orange : Color(.tertiarySystemFill))
                        )
                        .onTapGesture {
                            if liveSession.snapshot.phase.isActive {
                                liveSession.cancelSession()
                            } else {
                                liveSession.startSession()
                            }
                        }
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 10)
                .background(Color(.systemBackground))

                Divider()

                List {
                    // Defaults — always visible, pickers show message when empty
                    Section("Defaults") {
                        Picker("Voice Model", selection: sttBinding) {
                            if settings.allSTTModels.isEmpty {
                                Text("No providers configured").tag(noAPIKeyOption)
                            }
                            ForEach(sttPickerOptions, id: \.self) { option in
                                Text(option.displayName).tag(option)
                            }
                        }

                        Toggle("LLM Text Cleanup", isOn: $settings.llmEnabled)

                        if settings.llmEnabled {
                            Picker("LLM Model", selection: llmBinding) {
                                if settings.allLLMModels.isEmpty {
                                    Text("No providers configured").tag(noAPIKeyOption)
                                }
                                ForEach(llmPickerOptions, id: \.self) { option in
                                    Text(option.displayName).tag(option)
                                }
                            }
                        }

                        NavigationLink("System Prompt") {
                            PromptEditorView(title: "System Prompt", text: $settings.systemPrompt)
                        }
                    }

                    // Styles — cards with color accents
                    Section {
                        ForEach(Array(settings.styles.enumerated()), id: \.element.id) { index, style in
                            NavigationLink {
                                StyleEditView(style: $settings.styles[index])
                            } label: {
                                StyleCard(
                                    style: style,
                                    accentColor: styleColor(for: index),
                                    summary: styleSummary(style)
                                )
                            }
                        }
                        .onDelete { indexSet in
                            settings.styles.remove(atOffsets: indexSet)
                        }
                    } header: {
                        HStack {
                            Text("Styles")
                            Spacer()
                            Button {
                                settings.styles.append(
                                    DictationStyle(name: "", prompt: SettingsStore.defaultSystemPrompt)
                                )
                            } label: {
                                Image(systemName: "plus")
                                    .font(.caption2.weight(.bold))
                                    .foregroundStyle(.secondary)
                                    .frame(width: 22, height: 22)
                                    .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 6))
                            }
                            .buttonStyle(.plain)
                        }
                    }
                }
            }
            .toolbar(.hidden, for: .navigationBar)
        }
    }

    // MARK: - Sentinel for empty state

    private let noAPIKeyOption = ModelOption(provider: "", model: "")

    // MARK: - Picker Options (ensure current selection always has a tag)

    private var sttPickerOptions: [ModelOption] {
        let models = settings.allSTTModels
        let current = ModelOption(provider: settings.sttProvider, model: settings.sttModel)
        if models.contains(current) || models.isEmpty { return models }
        return [current] + models
    }

    private var llmPickerOptions: [ModelOption] {
        let models = settings.allLLMModels
        let current = ModelOption(provider: settings.llmProvider, model: settings.llmModel)
        if models.contains(current) || models.isEmpty { return models }
        return [current] + models
    }

    // MARK: - Unified Bindings

    private var sttBinding: Binding<ModelOption> {
        Binding(
            get: {
                if settings.allSTTModels.isEmpty { return noAPIKeyOption }
                return ModelOption(provider: settings.sttProvider, model: settings.sttModel)
            },
            set: { newValue in
                guard newValue != noAPIKeyOption else { return }
                settings.sttProvider = newValue.provider
                settings.sttModel = newValue.model
            }
        )
    }

    private var llmBinding: Binding<ModelOption> {
        Binding(
            get: {
                if settings.allLLMModels.isEmpty { return noAPIKeyOption }
                return ModelOption(provider: settings.llmProvider, model: settings.llmModel)
            },
            set: { newValue in
                guard newValue != noAPIKeyOption else { return }
                settings.llmProvider = newValue.provider
                settings.llmModel = newValue.model
            }
        )
    }

    // MARK: - Style Colors

    private static let accentColors: [Color] = [.blue, .purple, .orange, .pink, .teal, .indigo]

    private func styleColor(for index: Int) -> Color {
        Self.accentColors[index % Self.accentColors.count]
    }

    // MARK: - Helpers

    private func styleSummary(_ style: DictationStyle) -> String {
        let sttModel = style.sttModel ?? settings.sttModel
        let sttProvider = ProviderInfo.info(for: style.sttProvider ?? settings.sttProvider).displayName

        if settings.llmEnabled, let lm = style.llmModel ?? (settings.llmModel.isEmpty ? nil : settings.llmModel) {
            let llmProvider = ProviderInfo.info(for: style.llmProvider ?? settings.llmProvider).displayName
            return "\(sttModel) (\(sttProvider)) + \(lm) (\(llmProvider))"
        }
        return "\(sttModel) (\(sttProvider))"
    }
}

// MARK: - Style Card

private struct StyleCard: View {
    let style: DictationStyle
    let accentColor: Color
    let summary: String

    var body: some View {
        HStack(spacing: 12) {
            // Color accent strip
            RoundedRectangle(cornerRadius: 2)
                .fill(accentColor)
                .frame(width: 4, height: 40)

            VStack(alignment: .leading, spacing: 4) {
                Text(style.name.isEmpty ? "Untitled" : style.name)
                    .font(.body.weight(.medium))

                Text(summary)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
            }
        }
        .padding(.vertical, 2)
    }
}

// MARK: - Mini Waveform

private struct MiniWaveform: View {
    var compact: Bool = false

    @State private var animating = false

    private var barCount: Int { compact ? 20 : 32 }
    private var minHeight: CGFloat { compact ? 2 : 3 }
    private var maxHeight: CGFloat { compact ? 14 : 20 }
    private var barSpacing: CGFloat { compact ? 2 : 3 }
    private var barWidth: CGFloat { compact ? 3 : 4 }

    var body: some View {
        HStack(spacing: barSpacing) {
            ForEach(0..<barCount, id: \.self) { index in
                RoundedRectangle(cornerRadius: 1.5)
                    .fill(Color.accentColor.opacity(barOpacity(for: index)))
                    .frame(width: barWidth, height: animating ? barHeight(for: index) : minHeight)
                    .animation(
                        .easeInOut(duration: Double.random(in: 0.5...0.9))
                        .repeatForever(autoreverses: true)
                        .delay(Double(index) * 0.03),
                        value: animating
                    )
            }
        }
        .onAppear { animating = true }
    }

    private func barHeight(for index: Int) -> CGFloat {
        let normalized = Double(index) / Double(barCount - 1)
        let envelope = sin(normalized * .pi)
        return minHeight + (maxHeight - minHeight) * envelope * Double.random(in: 0.3...1.0)
    }

    private func barOpacity(for index: Int) -> Double {
        let normalized = Double(index) / Double(barCount - 1)
        return 0.25 + 0.6 * sin(normalized * .pi)
    }
}

// MARK: - Style Edit View

struct StyleEditView: View {
    @Binding var style: DictationStyle
    @Environment(SettingsStore.self) private var settings

    private let noAPIKeyOption = ModelOption(provider: "", model: "")

    var body: some View {
        Form {
            TextField("Name", text: $style.name)

            NavigationLink("Prompt") {
                PromptEditorView(title: style.name.isEmpty ? "Prompt" : style.name, text: $style.prompt)
            }

            Section("Model Overrides") {
                Picker("Voice Model", selection: sttOverrideBinding) {
                    Text("Default (\(settings.sttModel))").tag(Optional<ModelOption>.none)
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
        settings.llmModel.isEmpty ? "Default" : "Default (\(settings.llmModel))"
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

// MARK: - Prompt Editor

struct PromptEditorView: View {
    let title: String
    @Binding var text: String

    var body: some View {
        TextEditor(text: $text)
            .padding(12)
            .navigationTitle(title)
            .navigationBarTitleDisplayMode(.inline)
    }
}
