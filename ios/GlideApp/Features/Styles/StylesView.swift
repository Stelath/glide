import SwiftUI
import UIKit

// MARK: - Styles Tab

struct StylesView: View {
    @Environment(SettingsStore.self) private var settings
    @Environment(LiveDictationManager.self) private var liveSession

    @State private var showSTTPicker = false
    @State private var showLLMPicker = false

    var body: some View {
        @Bindable var settings = settings

        NavigationStack {
            VStack(spacing: 0) {
                // Custom top bar
                HStack {
                    // Invisible counter-balance to perfectly center the wordmark
                    Color.clear
                        .frame(width: 34, height: 34)

                    Spacer()

                    GlideWordmark()

                    Spacer()

                    Image(systemName: "mic.fill")
                        .font(.subheadline.weight(.medium))
                        .foregroundStyle(liveSession.snapshot.phase.isActive ? .white : .secondary)
                        .frame(width: 34, height: 34)
                        .background(
                            RoundedRectangle(cornerRadius: 8)
                                .fill(liveSession.snapshot.phase.isActive ? settings.accent.primary : settings.accent.accentSurface)
                        )
                        .onTapGesture {
                            UIImpactFeedbackGenerator(style: .medium).impactOccurred()
                            if liveSession.snapshot.phase.isActive {
                                liveSession.cancelSession()
                            } else {
                                liveSession.startSession()
                            }
                        }
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 10)

                Divider()

                List {
                    // Defaults — always visible, rows open bottom sheet pickers
                    Section("Defaults") {
                        Button {
                            if !sttPickerOptions.isEmpty { showSTTPicker = true }
                        } label: {
                            HStack {
                                Text("Voice Model")
                                Spacer()
                                Text(sttPickerOptions.isEmpty ? "No providers" : sttBinding.wrappedValue.displayName)
                                    .foregroundStyle(.secondary)
                            }
                        }
                        .foregroundStyle(.primary)

                        Button {
                            if !llmPickerOptions.isEmpty { showLLMPicker = true }
                        } label: {
                            HStack {
                                Text("LLM Model")
                                Spacer()
                                Text(llmPickerOptions.isEmpty ? "No providers" : llmBinding.wrappedValue.displayName)
                                    .foregroundStyle(.secondary)
                            }
                        }
                        .foregroundStyle(.primary)

                        NavigationLink("System Prompt") {
                            PromptEditorView(title: "System Prompt", text: $settings.systemPrompt)
                        }
                    }

                    // Styles — cards with color accents
                    Section {
                        ForEach(Array(settings.styles.enumerated()), id: \.element.id) { index, style in
                            NavigationLink {
                                StyleEditView(style: styleBinding(for: style))
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
                                    .background(Color.glideSurface, in: RoundedRectangle(cornerRadius: 6))
                            }
                            .buttonStyle(.plain)
                        }
                    }
                }
            }
            .toolbar(.hidden, for: .navigationBar)
            .background(Color.glideBackground)
            .scrollContentBackground(.hidden)
            .sheet(isPresented: $showSTTPicker) {
                ModelPickerSheet(title: "Voice Model", options: sttPickerOptions, selection: sttBinding)
            }
            .sheet(isPresented: $showLLMPicker) {
                ModelPickerSheet(title: "LLM Model", options: llmPickerOptions, selection: llmBinding)
            }
        }
    }

    // MARK: - Picker Options

    private var sttPickerOptions: [ModelOption] {
        settings.allSTTModels
    }

    private var llmPickerOptions: [ModelOption] {
        settings.allLLMModels
    }

    // MARK: - Unified Bindings

    private var sttBinding: Binding<ModelOption> {
        Binding(
            get: {
                let current = ModelOption(provider: settings.sttProvider, model: settings.sttModel)
                if sttPickerOptions.contains(current) { return current }
                return sttPickerOptions.first ?? ModelOption(provider: "", model: "")
            },
            set: { newValue in
                settings.sttProvider = newValue.provider
                settings.sttModel = newValue.model
            }
        )
    }

    private var llmBinding: Binding<ModelOption> {
        Binding(
            get: {
                let current = ModelOption(provider: settings.llmProvider, model: settings.llmModel)
                if llmPickerOptions.contains(current) { return current }
                return llmPickerOptions.first ?? ModelOption(provider: "", model: "")
            },
            set: { newValue in
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

        if let lm = style.llmModel ?? (settings.llmModel.isEmpty ? nil : settings.llmModel) {
            let llmProvider = ProviderInfo.info(for: style.llmProvider ?? settings.llmProvider).displayName
            return "\(sttModel) (\(sttProvider)) + \(lm) (\(llmProvider))"
        }
        return "\(sttModel) (\(sttProvider))"
    }

    private func styleBinding(for style: DictationStyle) -> Binding<DictationStyle> {
        Binding(
            get: {
                settings.styles.first(where: { $0.id == style.id }) ?? style
            },
            set: { updatedStyle in
                guard let index = settings.styles.firstIndex(where: { $0.id == style.id }) else { return }
                settings.styles[index] = updatedStyle
            }
        )
    }
}

// MARK: - Bottom Sheet Picker

private struct ModelPickerSheet: View {
    let title: String
    let options: [ModelOption]
    @Binding var selection: ModelOption
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            Picker(title, selection: $selection) {
                ForEach(options, id: \.self) { option in
                    Text(option.displayName).tag(option)
                }
            }
            .pickerStyle(.wheel)
            .navigationTitle(title)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("Done") { dismiss() }
                }
            }
        }
        .presentationDetents([.medium])
    }
}
