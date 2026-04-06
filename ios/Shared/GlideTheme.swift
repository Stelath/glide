import SwiftUI

extension Color {
    // MARK: - Core Palette

    /// App background — #F0F7FF
    static let glideBackground = Color(red: 240 / 255, green: 247 / 255, blue: 255 / 255)

    /// Surface/card background — #D6EAFF
    static let glideSurface = Color(red: 214 / 255, green: 234 / 255, blue: 255 / 255)

    /// Primary interactive color — #4A9FE5
    static let glidePrimary = Color(red: 74 / 255, green: 159 / 255, blue: 229 / 255)

    /// Primary text color — #1B3A5C
    static let glideText = Color(red: 27 / 255, green: 58 / 255, blue: 92 / 255)

    // MARK: - Accent Palette

    /// Accent surface (idle/unselected backgrounds) — #E3DEEF
    static let glideAccentSurface = Color(red: 227 / 255, green: 222 / 255, blue: 239 / 255)

    /// Accent mid (secondary text/icons) — #B8ACE0
    static let glideAccentMid = Color(red: 184 / 255, green: 172 / 255, blue: 224 / 255)

    /// Accent bold (style card accents, active accent) — #7E6CC4
    static let glideAccentBold = Color(red: 126 / 255, green: 108 / 255, blue: 196 / 255)

    /// Accent dark (nav titles, dark emphasis) — #2D2548
    static let glideAccentDark = Color(red: 45 / 255, green: 37 / 255, blue: 72 / 255)

    // MARK: - Style Accent Palette

    /// 6 shades within the accent spectrum for style cards and keyboard pills
    static let glideStyleAccents: [Color] = [
        glideAccentBold,
        Color(red: 106 / 255, green: 89 / 255, blue: 173 / 255),
        Color(red: 140 / 255, green: 120 / 255, blue: 200 / 255),
        Color(red: 90 / 255, green: 75 / 255, blue: 155 / 255),
        Color(red: 160 / 255, green: 140 / 255, blue: 210 / 255),
        glideAccentDark,
    ]
}
