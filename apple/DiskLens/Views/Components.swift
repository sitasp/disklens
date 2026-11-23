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

struct PhaseLabel: View {
    var phase: ScanPhase
    var body: some View {
        Text(title)
    }

    private var title: String {
        switch phase {
        case .walking: return "Reading folders"
        case .finalizing: return "Adding up sizes"
        case .grouping: return "Grouping by size"
        case .hashingPartial: return "Comparing file edges"
        case .hashingFull: return "Hashing full files"
        case .done: return "Finishing"
        }
    }
}
