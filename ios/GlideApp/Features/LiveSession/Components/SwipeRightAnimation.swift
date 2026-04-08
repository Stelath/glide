import SwiftUI

struct SwipeRightAnimation: View {
    @State private var offset: CGFloat = -40
    @State private var opacity: Double = 0

    var body: some View {
        ZStack {
            Capsule()
                .fill(.secondary.opacity(0.15))
                .frame(width: 120, height: 4)

            Image(systemName: "hand.point.right.fill")
                .font(.title2)
                .foregroundStyle(.secondary)
                .offset(x: offset)
                .opacity(opacity)
        }
        .onAppear {
            startAnimation()
        }
    }

    private func startAnimation() {
        offset = -40
        opacity = 0

        withAnimation(.easeIn(duration: 0.3)) {
            opacity = 1
        }

        withAnimation(.easeInOut(duration: 1.0).delay(0.2)) {
            offset = 40
        }

        withAnimation(.easeOut(duration: 0.3).delay(1.0)) {
            opacity = 0
        }

        DispatchQueue.main.asyncAfter(deadline: .now() + 1.8) {
            startAnimation()
        }
    }
}
