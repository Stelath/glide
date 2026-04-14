import SwiftUI

struct ContentView: View {
    var body: some View {
        TabView {
            StylesView()
                .overlay(alignment: .bottomTrailing) { AccountChip() }
                .tabItem {
                    Label("Styles", systemImage: "paintbrush")
                }

            ProvidersView()
                .overlay(alignment: .bottomTrailing) { AccountChip() }
                .tabItem {
                    Label("Providers", systemImage: "key")
                }

            GeneralView()
                .overlay(alignment: .bottomTrailing) { AccountChip() }
                .tabItem {
                    Label("General", systemImage: "gear")
                }
        }
    }
}
