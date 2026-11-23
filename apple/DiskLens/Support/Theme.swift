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
        case .overview: return "Dashboard"
        case .map: return "Disk Map"
        case .categories: return "Categories"
        case .largeFiles: return "Large Files"
        case .duplicates: return "Duplicates"
        case .history: return "History"
        }
    }

    var symbol: String {
        switch self {
        case .overview: return "square.grid.2x2.fill"
        case .map: return "chart.pie.fill"
        case .categories: return "square.stack.3d.up.fill"
        case .largeFiles: return "doc.fill"
        case .duplicates: return "square.on.square.fill"
        case .history: return "clock.arrow.circlepath"
        }
    }
}

extension FileCategory {
    var symbol: String {
        switch self {
        case .application: return "square.stack.3d.up.fill"
        case .audio: return "waveform"
        case .video: return "film.fill"
        case .image: return "photo.fill"
        case .document: return "doc.richtext.fill"
        case .archive: return "archivebox.fill"
        case .code: return "chevron.left.forwardslash.chevron.right"
        case .developerCache: return "shippingbox.fill"
        case .system: return "gearshape.fill"
        case .other: return "square.dashed"
        }
    }
}

enum DashboardVolumes {
    /// The disk the dashboard leads with: a fixed volume that reported usage,
    /// otherwise any volume that reported usage, otherwise the first card.
    static func featured(_ volumes: [VolumeInfo]) -> VolumeInfo? {
        if let fixed = volumes.first(where: { !$0.removable && $0.usage != nil }) {
            return fixed
        }
        if let measured = volumes.first(where: { $0.usage != nil }) {
            return measured
        }
        return volumes.first
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
