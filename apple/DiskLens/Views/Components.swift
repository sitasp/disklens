import SwiftUI

struct ByteText: View {
    var bytes: UInt64
    var emphasis: Bool = false

    var body: some View {
        Text(formatByteCount(bytes: bytes))
            .monospacedDigit()
            .fontWeight(emphasis ? .semibold : .regular)
            .accessibilityLabel(formatByteCount(bytes: bytes))
    }
}

struct UsageBar: View {
    var used: Double
    var total: Double

    var body: some View {
        GeometryReader { geo in
            let fraction = total > 0 ? min(max(used / total, 0), 1) : 0
            ZStack(alignment: .leading) {
                Capsule().fill(Color.primary.opacity(0.08))
                Capsule()
                    .fill(Palette.accent)
                    .frame(width: max(4, geo.size.width * fraction))
            }
        }
        .frame(height: 8)
        .accessibilityLabel("Used space")
        .accessibilityValue(total > 0 ? "\(Int((used / total) * 100)) percent" : "Unknown")
    }
}

struct EmptyState: View {
    var title: String
    var message: String
    var symbol: String
    var actionTitle: String?
    var action: (() -> Void)?

    var body: some View {
        VStack(spacing: 12) {
            Image(systemName: symbol)
                .font(.system(size: 36, weight: .light))
                .foregroundStyle(Palette.accent)
            Text(title)
                .font(.title3.weight(.semibold))
            Text(message)
                .font(.callout)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
                .frame(maxWidth: 420)
            if let actionTitle, let action {
                Button(actionTitle, action: action)
                    .keyboardShortcut(.defaultAction)
                    .padding(.top, 4)
            }
        }
        .padding(32)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

struct CategorySwatch: View {
    var category: FileCategory
    var body: some View {
        Circle()
            .fill(Palette.color(for: category))
            .frame(width: 8, height: 8)
            .accessibilityHidden(true)
    }
}

/// Progress for a scan or a duplicate search. The column has a fixed width
/// and the path keeps two lines, so changing counts and paths do not move
/// the title or the Stop button.
struct ScanActivityPanel: View {
    var phase: ScanPhase?
    var stats: String
    var path: String
    var stop: () -> Void
    var stopLabel: String

    var body: some View {
        VStack(spacing: 14) {
            ProgressView()
                .controlSize(.large)
            Text(phase.map(PhaseLabel.title(for:)) ?? "Starting")
                .font(.headline)
                .lineLimit(1)
                .frame(maxWidth: .infinity)
            Text(stats)
                .foregroundStyle(.secondary)
                .monospacedDigit()
                .lineLimit(1)
                .frame(maxWidth: .infinity)
            Text(path.isEmpty ? " " : path)
                .font(.caption)
                .foregroundStyle(.secondary)
                .lineLimit(2, reservesSpace: true)
                .truncationMode(.middle)
                .multilineTextAlignment(.center)
                .frame(maxWidth: .infinity, alignment: .top)
            Button("Stop", action: stop)
                .accessibilityLabel(stopLabel)
        }
        .frame(width: 520)
        .padding(32)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .transaction { $0.disablesAnimations = true }
    }
}

struct PhaseLabel: View {
    var phase: ScanPhase
    var body: some View {
        Text(title)
    }

    fileprivate static func title(for phase: ScanPhase) -> String {
        switch phase {
        case .walking: return "Reading folders"
        case .finalizing: return "Adding up sizes"
        case .grouping: return "Grouping by size"
        case .hashingPartial: return "Comparing file edges"
        case .hashingFull: return "Hashing full files"
        case .done: return "Finishing"
        }
    }

    private var title: String {
        Self.title(for: phase)
    }
}
