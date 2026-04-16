import SwiftUI

struct ContentView: View {
    var body: some View {
        TabView {
            StylesView()
                .tabItem {
                    Label("Styles", systemImage: "paintbrush")
                }

            ProvidersView()
                .tabItem {
                    Label("Providers", systemImage: "key")
                }

            GeneralView()
                .tabItem {
                    Label("General", systemImage: "gear")
                }

            DictionaryView()
                .tabItem {
                    Label("Dictionary", systemImage: "book")
                }
        }
    }
}
