import SwiftUI

struct CategoryView: View {
    var model: AppModel

    var body: some View {
        Group {
            if model.session == nil {
                EmptyState(
                    title: "No scan yet",
                    message: "Categories are counted from a finished map. Scan a folder first, then come back to see photos, caches, and archives side by side.",
                    symbol: "square.grid.2x2",
                    actionTitle: "Open Folder…",
                    action: { model.chooseFolder(stayOnFilesystem: false) }
                )
            } else if model.categories.isEmpty {
                EmptyState(
                    title: "No files in this scan",
                    message: "The folder was readable, but it did not contain any regular files to classify.",
                    symbol: "doc",
                    actionTitle: nil,
                    action: nil
                )
            } else {
                list
            }
        }
        .background(Color(nsColor: .windowBackgroundColor))
    }

    private var list: some View {
        let peak = model.categories.map(\.uniqueApparent).max() ?? 1
        return ScrollView {
            VStack(alignment: .leading, spacing: 14) {
                Text("Unique bytes by kind of file. A JavaScript file inside node_modules counts as a developer cache, not as code. Files inside an app bundle count as the app.")
                    .font(.title3)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
                ForEach(model.categories, id: \.category) { row in
                    VStack(alignment: .leading, spacing: 10) {
                        HStack(spacing: 14) {
                            CategoryMark(category: row.category)
                            Text(row.label)
                                .font(.title2.weight(.semibold))
                            Spacer()
                            ByteText(bytes: row.uniqueApparent, emphasis: true)
                                .font(.title2)
                        }
                        GeometryReader { geo in
                            let fraction = peak > 0 ? CGFloat(row.uniqueApparent) / CGFloat(peak) : 0
                            ZStack(alignment: .leading) {
                                Capsule().fill(Color.primary.opacity(0.06))
                                Capsule()
                                    .fill(Palette.color(for: row.category))
                                    .frame(width: max(4, geo.size.width * fraction))
                            }
                        }
                        .frame(height: 10)
                        Text("\(row.fileCount) files · \(formatByteCount(bytes: row.uniqueAllocated)) allocated")
                            .font(.title3)
                            .foregroundStyle(.secondary)
                            .monospacedDigit()
                    }
                    .padding(14)
                    .background(.background, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
                    .accessibilityElement(children: .combine)
                    .accessibilityLabel("\(row.label), \(formatByteCount(bytes: row.uniqueApparent)), \(row.fileCount) files")
                }
            }
            .padding(24)
            .frame(maxWidth: 760, alignment: .leading)
        }
    }
}
