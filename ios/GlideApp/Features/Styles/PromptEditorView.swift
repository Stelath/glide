import SwiftUI

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
