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
        VStack(spacing: 18) {
            Image(systemName: symbol)
                .font(.system(size: 34, weight: .semibold))
                .foregroundStyle(Palette.accent)
                .frame(width: 84, height: 84)
                .background(Palette.accent.opacity(0.16), in: Circle())
            Text(title)
                .font(.largeTitle.weight(.semibold))
                .multilineTextAlignment(.center)
            Text(message)
                .font(.title3)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
                .frame(maxWidth: 560)
            if let actionTitle, let action {
                Button(actionTitle, action: action)
                    .buttonStyle(.borderedProminent)
                    .controlSize(.large)
                    .keyboardShortcut(.defaultAction)
                    .padding(.top, 8)
            }
        }
        .padding(40)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

struct CategorySwatch: View {
    var category: FileCategory
    var body: some View {
        Circle()
            .fill(Palette.color(for: category))
            .frame(width: 10, height: 10)
            .accessibilityHidden(true)
    }
}

struct CategoryMark: View {
    var category: FileCategory
    var size: CGFloat = 44

    var body: some View {
        Image(systemName: category.symbol)
            .font(.system(size: size * 0.38, weight: .semibold))
            .foregroundStyle(Palette.color(for: category))
            .frame(width: size, height: size)
            .background(Palette.color(for: category).opacity(0.18), in: Circle())
            .accessibilityHidden(true)
    }
}

/// Progress for a scan or a duplicate search. The column has a fixed width
/// and the path keeps two lines, so changing counts and paths do not move
/// the title or the Stop button.
struct ScanActivityPanel: View {
    var phase: ScanPhase?
    /// Shown in place of the figures until the first progress update.
    var waiting: String
    var count: String
    var countLabel: String
    var amount: String
    var amountLabel: String
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
            ZStack {
                figures
                    .opacity(phase == nil ? 0 : 1)
                if phase == nil {
                    Text(waiting)
                        .foregroundStyle(.secondary)
                }
            }
            .frame(height: 20)
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

    /// Each number sits in its own slot, aligned to the trailing edge, so a
    /// new digit grows left and the words beside it do not move.
    private var figures: some View {
        HStack(spacing: 0) {
            slotted(count, width: 132)
            Text(countLabel)
            slotted(amount, width: 88)
            Text(amountLabel)
        }
        .foregroundStyle(.secondary)
        .lineLimit(1)
    }

    private func slotted(_ text: String, width: CGFloat) -> some View {
        Text(text)
            .monospacedDigit()
            .frame(width: width, alignment: .trailing)
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
