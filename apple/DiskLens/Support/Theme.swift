import SwiftUI

enum Palette {
    static func color(for category: FileCategory) -> Color {
        switch category {
        case .application:
            return Color(red: 0.36, green: 0.55, blue: 0.94)
        case .audio:
            return Color(red: 0.76, green: 0.42, blue: 0.84)
        case .video:
            return Color(red: 0.89, green: 0.36, blue: 0.27)
        case .image:
            return Color(red: 0.22, green: 0.70, blue: 0.48)
        case .document:
            return Color(red: 0.86, green: 0.62, blue: 0.24)
        case .archive:
            return Color(red: 0.55, green: 0.43, blue: 0.39)
        case .code:
            return Color(red: 0.28, green: 0.55, blue: 0.54)
        case .developerCache:
            return Color(red: 0.82, green: 0.40, blue: 0.58)
        case .system:
            return Color(red: 0.55, green: 0.56, blue: 0.60)
        case .other:
            return Color(red: 0.42, green: 0.48, blue: 0.56)
        }
    }

    static let accent = Color(red: 0.18, green: 0.62, blue: 0.62)
}

enum SidebarSection: String, CaseIterable, Identifiable, Hashable {
    case overview
    case map
    case categories
    case largeFiles
    case duplicates
    case history

    var id: String { rawValue }

    var title: String {
        switch self {
        case .overview: return "Overview"
        case .map: return "Disk Map"
        case .categories: return "Categories"
        case .largeFiles: return "Large Files"
        case .duplicates: return "Duplicates"
        case .history: return "History"
        }
    }

    var symbol: String {
        switch self {
        case .overview: return "internaldrive"
        case .map: return "circle.hexagongrid"
        case .categories: return "square.grid.2x2"
        case .largeFiles: return "doc.text.magnifyingglass"
        case .duplicates: return "square.on.square"
        case .history: return "clock.arrow.circlepath"
        }
    }
}

enum MapVisualization: String, CaseIterable, Identifiable {
    case sunburst
    case treemap

    var id: String { rawValue }

    var title: String {
        switch self {
        case .sunburst: return "Sunburst"
        case .treemap: return "Treemap"
        }
    }
}
