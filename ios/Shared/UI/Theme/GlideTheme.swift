import SwiftUI

// MARK: - Accent Theme

enum GlideAccent: String, CaseIterable, Codable, Identifiable, Sendable {
    case purple, blue, orange, slate

    var id: String { rawValue }

    var displayName: String {
        switch self {
        case .purple: return "Purple"
        case .blue: return "Powder Blue"
        case .orange: return "Orange"
        case .slate: return "Slate"
        }
    }

    var primary: Color {
        switch self {
        case .purple: return Color(red: 126 / 255, green: 108 / 255, blue: 196 / 255)
        case .blue: return Color(red: 74 / 255, green: 143 / 255, blue: 212 / 255)
        case .orange: return Color(red: 240 / 255, green: 96 / 255, blue: 58 / 255)
        case .slate: return Color(red: 107 / 255, green: 114 / 255, blue: 128 / 255)
        }
    }

    var accentSurface: Color {
        let uiColor = UIColor { [self] traits -> UIColor in
            if traits.userInterfaceStyle == .dark {
                return self.accentSurfaceDarkUIColor
            } else {
                return self.accentSurfaceLightUIColor
            }
        }
        return Color(uiColor: uiColor)
    }

    private var accentSurfaceLightUIColor: UIColor {
        switch self {
        case .purple: return UIColor(red: 227 / 255, green: 222 / 255, blue: 239 / 255, alpha: 1)
        case .blue: return UIColor(red: 220 / 255, green: 232 / 255, blue: 245 / 255, alpha: 1)
        case .orange: return UIColor(red: 252 / 255, green: 224 / 255, blue: 214 / 255, alpha: 1)
        case .slate: return UIColor(red: 229 / 255, green: 231 / 255, blue: 235 / 255, alpha: 1)
        }
    }

    private var accentSurfaceDarkUIColor: UIColor {
        switch self {
        case .purple: return UIColor(red: 60 / 255, green: 50 / 255, blue: 90 / 255, alpha: 1)
        case .blue: return UIColor(red: 42 / 255, green: 62 / 255, blue: 90 / 255, alpha: 1)
        case .orange: return UIColor(red: 90 / 255, green: 36 / 255, blue: 24 / 255, alpha: 1)
        case .slate: return UIColor(red: 55 / 255, green: 65 / 255, blue: 81 / 255, alpha: 1)
        }
    }

    var accentMid: Color {
        switch self {
        case .purple: return Color(red: 184 / 255, green: 172 / 255, blue: 224 / 255)
        case .blue: return Color(red: 160 / 255, green: 196 / 255, blue: 232 / 255)
        case .orange: return Color(red: 245 / 255, green: 160 / 255, blue: 138 / 255)
        case .slate: return Color(red: 156 / 255, green: 163 / 255, blue: 175 / 255)
        }
    }

    var accentDark: Color {
        switch self {
        case .purple: return Color(red: 45 / 255, green: 37 / 255, blue: 72 / 255)
        case .blue: return Color(red: 30 / 255, green: 52 / 255, blue: 86 / 255)
        case .orange: return Color(red: 107 / 255, green: 38 / 255, blue: 22 / 255)
        case .slate: return Color(red: 31 / 255, green: 41 / 255, blue: 55 / 255)
        }
    }

    var styleAccents: [Color] {
        switch self {
        case .purple:
            return [
                primary,
                Color(red: 106 / 255, green: 89 / 255, blue: 173 / 255),
                Color(red: 140 / 255, green: 120 / 255, blue: 200 / 255),
                Color(red: 90 / 255, green: 75 / 255, blue: 155 / 255),
                Color(red: 160 / 255, green: 140 / 255, blue: 210 / 255),
                accentDark,
            ]
        case .blue:
            return [
                primary,
                Color(red: 60 / 255, green: 120 / 255, blue: 190 / 255),
                Color(red: 100 / 255, green: 160 / 255, blue: 220 / 255),
                Color(red: 50 / 255, green: 100 / 255, blue: 165 / 255),
                Color(red: 130 / 255, green: 180 / 255, blue: 230 / 255),
                accentDark,
            ]
        case .orange:
            return [
                primary,
                Color(red: 215 / 255, green: 78 / 255, blue: 48 / 255),
                Color(red: 245 / 255, green: 130 / 255, blue: 100 / 255),
                Color(red: 190 / 255, green: 65 / 255, blue: 40 / 255),
                Color(red: 248 / 255, green: 155 / 255, blue: 130 / 255),
                accentDark,
            ]
        case .slate:
            return [
                primary,
                Color(red: 90 / 255, green: 97 / 255, blue: 110 / 255),
                Color(red: 130 / 255, green: 137 / 255, blue: 150 / 255),
                Color(red: 75 / 255, green: 82 / 255, blue: 95 / 255),
                Color(red: 150 / 255, green: 157 / 255, blue: 170 / 255),
                accentDark,
            ]
        }
    }

    var iconName: String? {
        switch self {
        case .purple: return nil
        case .blue: return "AppIcon-Blue"
        case .orange: return "AppIcon-Orange"
        case .slate: return "AppIcon-Slate"
        }
    }

    /// Read current accent from app group store (for extensions without SettingsStore)
    static var current: GlideAccent {
        guard let defaults = UserDefaults(suiteName: "group.com.stelath.glide.app"),
              let raw = defaults.string(forKey: "accent") else {
            return .slate
        }
        return GlideAccent(rawValue: raw) ?? .slate
    }
}

// MARK: - Fixed Colors (same across all themes)

extension Color {
    /// App background — light: #FAF9F6 (warm cream), dark: #111111
    static let glideBackground: Color = {
        let uiColor = UIColor { traits in
            if traits.userInterfaceStyle == .dark {
                return UIColor(red: 17 / 255, green: 17 / 255, blue: 17 / 255, alpha: 1)
            } else {
                return UIColor(red: 250 / 255, green: 249 / 255, blue: 246 / 255, alpha: 1)
            }
        }
        return Color(uiColor: uiColor)
    }()

    /// Surface/card background — light: #EDE9E3, dark: #2C2C2E
    static let glideSurface: Color = {
        let uiColor = UIColor { traits in
            if traits.userInterfaceStyle == .dark {
                return UIColor(red: 44 / 255, green: 44 / 255, blue: 46 / 255, alpha: 1)
            } else {
                return UIColor(red: 237 / 255, green: 233 / 255, blue: 227 / 255, alpha: 1)
            }
        }
        return Color(uiColor: uiColor)
    }()

    /// Primary text color — light: #2D2A25, dark: #F5F5F5
    static let glideText: Color = {
        let uiColor = UIColor { traits in
            if traits.userInterfaceStyle == .dark {
                return UIColor(red: 245 / 255, green: 245 / 255, blue: 245 / 255, alpha: 1)
            } else {
                return UIColor(red: 45 / 255, green: 42 / 255, blue: 37 / 255, alpha: 1)
            }
        }
        return Color(uiColor: uiColor)
    }()
}
