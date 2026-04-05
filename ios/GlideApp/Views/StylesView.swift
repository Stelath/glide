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

// MARK: - Previews

#Preview("Styles") {
    let settings = SettingsStore(preview: true)
    settings.llmEnabled = true
    settings.sttProvider = "openai"
    settings.sttModel = "gpt-4o-mini-transcribe"
    settings.llmProvider = "openai"
    settings.llmModel = "gpt-4.1-mini"
    settings.systemPrompt = "You are a helpful dictation assistant."
    settings.styles = [
        DictationStyle(
            name: "Clinical",
            prompt: "Rewrite as a concise clinical note.",
            sttProvider: nil,
            sttModel: nil,
            llmProvider: nil,
            llmModel: nil
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
            prompt: "Rewrite as a concise clinical note.",
            sttProvider: nil,
            sttModel: nil,
            llmProvider: nil,
            llmModel: nil
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

// MARK: - Glide Wordmark

/// Vertical bars positioned at different offsets to spell "glide" in lowercase.
/// Audio visualizer effect: each segment scales upward and downward dynamically from its exact center.
struct GlideWordmark: View {
    private static let columns: [[(bottom: CGFloat, top: CGFloat)]] = buildColumns()
    
    // Increased base sizes to make the logo bolder and give room for movement
    private let frameHeight: CGFloat = 40
    private let barWidth: CGFloat = 4.0
    private let barSpacing: CGFloat = 2.0
    private let gapWidth: CGFloat = 6.0
    
    var body: some View {
        TimelineView(.animation(minimumInterval: 1.0 / 60.0)) { timeline in
            let now = timeline.date.timeIntervalSinceReferenceDate
            HStack(alignment: .bottom, spacing: barSpacing) {
                ForEach(0..<Self.columns.count, id: \.self) { col in
                    let segments = Self.columns[col]
                    if segments.isEmpty {
                        Spacer().frame(width: gapWidth)
                    } else {
                        ZStack(alignment: .bottom) {
                            ForEach(0..<segments.count, id: \.self) { seg in
                                let s = segments[seg]
                                
                                // Significantly slowed down waves for a smoother, relaxed pulse
                                let primaryWave = sin(now * 6.0 + Double(col) * 1.5)
                                let secondaryWave = cos(now * 9.0 + Double(seg) * 3.1)
                                
                                // Combine the waves into a roughly -1.0 to 1.0 range
                                let combinedWave = (primaryWave * 0.6) + (secondaryWave * 0.4)
                                
                                // Define a fixed absolute maximum expansion in points (e.g., +/- 4 points)
                                let maxExpansion: CGFloat = 4.0
                                let expansion = CGFloat(combinedWave) * maxExpansion
                                
                                // Calculate standard height in points
                                let originalHeight = (s.top - s.bottom) * frameHeight
                                
                                // Apply the fixed point expansion instead of a percentage
                                let h = max(2, originalHeight + expansion)
                                
                                // To pulse from the center, we calculate the difference in height caused by the stretch
                                // and push the bottom offset down by exactly half of that difference.
                                let heightDiff = h - originalHeight
                                let centerOffset = (s.bottom * frameHeight) - (heightDiff / 2)
                                
                                RoundedRectangle(cornerRadius: barWidth / 2)
                                    .fill(Color.accentColor.opacity(0.4 + 0.6 * s.top))
                                    .frame(width: barWidth, height: h)
                                    .offset(y: -centerOffset) // Pulses outward from the center of the segment
                            }
                        }
                        .frame(height: frameHeight, alignment: .bottom)
                    }
                }
            }
        }
    }
    
    // MARK: - Letter Definitions
    
    /// Builds the full column array for "glide".
    /// Coordinates: 0 = frame bottom, 1 = frame top.
    /// Baseline ≈ 0.15, x-height ≈ 0.70, ascender ≈ 1.00, descender ≈ 0.00.
    private static func buildColumns() -> [[(bottom: CGFloat, top: CGFloat)]] {
        let g: [[(bottom: CGFloat, top: CGFloat)]] = [
            [(0.10, 0.20), (0.30, 0.68)],               // Tail hook + left side of bowl (shifted up and stretched)
            [(0.04, 0.10), (0.20, 0.30), (0.68, 0.78)], // Tail curve + top & bottom of bowl
            [(0.04, 0.10), (0.20, 0.30), (0.68, 0.78)], // Tail curve + top & bottom of bowl (fixed connection)
            [(0.04, 0.78)],                             // Right stem completely down to tail curve
        ]
        
        let l: [[(bottom: CGFloat, top: CGFloat)]] = [
            [(0.15, 1.00)]                              // Single, strong tall stroke
        ]
        
        let i: [[(bottom: CGFloat, top: CGFloat)]] = [
            [(0.15, 0.70), (0.85, 0.95)]                // Stem + dot
        ]
        
        let d: [[(bottom: CGFloat, top: CGFloat)]] = [
            [(0.25, 0.60)],                             // Leftmost edge of bowl
            [(0.15, 0.25), (0.60, 0.70)],               // Top & bottom of bowl
            [(0.15, 0.25), (0.60, 0.70)],               // Top & bottom of bowl
            [(0.15, 1.00)]                              // Ascender (right stem)
        ]
        
        let e: [[(bottom: CGFloat, top: CGFloat)]] = [
            [(0.20, 0.65)],                             // Smooth left back curve
            [(0.15, 0.25), (0.45, 0.50), (0.65, 0.70)], // Bottom curve, middle crossbar, top curve
            [(0.15, 0.25), (0.45, 0.50), (0.65, 0.70)], // Bottom curve, middle crossbar, top curve
            [(0.45, 0.65)]                              // Caps the top loop, leaves bottom half open
        ]
        
        let gap: [[(bottom: CGFloat, top: CGFloat)]] = [[]]
        
        return g + gap + l + gap + i + gap + d + gap + e
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
