import SwiftUI

struct DictionaryView: View {
    @Environment(SettingsStore.self) private var settings

    @State private var newWord = ""
    @State private var newFind = ""
    @State private var newReplace = ""
    @State private var newCaseSensitive = false

    var body: some View {
        NavigationStack {
            VStack(spacing: 0) {
                HStack {
                    Text("Dictionary")
                        .font(.subheadline.weight(.semibold))
                        .foregroundStyle(Color.glideText)
                }
                .frame(maxWidth: .infinity)
                .padding(.vertical, 10)

                Divider()

                Form {
                    Section {
                        ForEach(settings.vocabulary, id: \.self) { word in
                            Text(word)
                        }
                        .onDelete { offsets in
                            settings.vocabulary.remove(atOffsets: offsets)
                        }

                        HStack {
                            TextField("Add word or phrase", text: $newWord)
                                .autocorrectionDisabled()
                                .textInputAutocapitalization(.never)
                            Button("Add") {
                                let trimmed = newWord.trimmingCharacters(in: .whitespacesAndNewlines)
                                guard !trimmed.isEmpty else { return }
                                settings.vocabulary.append(trimmed)
                                newWord = ""
                            }
                            .disabled(newWord.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                        }
                    } header: {
                        Text("Vocabulary")
                    } footer: {
                        Text("Words and phrases that help the transcription model recognize specific terms, names, and acronyms.")
                    }

                    Section {
                        ForEach(settings.replacements) { rule in
                            HStack(spacing: 6) {
                                Text(rule.find)
                                    .foregroundStyle(.primary)
                                Image(systemName: "arrow.right")
                                    .foregroundStyle(.secondary)
                                    .font(.caption)
                                Text(rule.replace)
                                    .foregroundStyle(.primary)
                                if rule.caseSensitive {
                                    Text("Aa")
                                        .font(.caption2)
                                        .foregroundStyle(.secondary)
                                }
                            }
                        }
                        .onDelete { offsets in
                            settings.replacements.remove(atOffsets: offsets)
                        }

                        VStack(spacing: 8) {
                            HStack {
                                TextField("Find", text: $newFind)
                                    .autocorrectionDisabled()
                                    .textInputAutocapitalization(.never)
                                TextField("Replace with", text: $newReplace)
                                    .autocorrectionDisabled()
                                    .textInputAutocapitalization(.never)
                            }
                            HStack {
                                Toggle("Case sensitive", isOn: $newCaseSensitive)
                                    .font(.caption)
                                Spacer()
                                Button("Add Rule") {
                                    addReplacement()
                                }
                                .disabled(newFind.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                            }
                        }
                    } header: {
                        Text("Replacements")
                    } footer: {
                        Text("Auto-replace rules applied to transcriptions. Useful for correcting common misheard words.")
                    }
                }
            }
            .toolbar(.hidden, for: .navigationBar)
            .background(Color.glideBackground)
            .scrollContentBackground(.hidden)
        }
    }

    private func addReplacement() {
        let find = newFind.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !find.isEmpty else { return }
        settings.replacements.append(ReplacementRule(
            find: find,
            replace: newReplace.trimmingCharacters(in: .whitespacesAndNewlines),
            caseSensitive: newCaseSensitive
        ))
        newFind = ""
        newReplace = ""
        newCaseSensitive = false
    }
}
