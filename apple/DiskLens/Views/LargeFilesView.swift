import SwiftUI

struct LargeFilesView: View {
    @Bindable var model: AppModel

    var body: some View {
        Group {
            if model.session == nil {
                EmptyState(
                    title: "No scan yet",
                    message: "Large and old files come from the map you just made. Scan a folder, then filter this list.",
                    symbol: "doc.text.magnifyingglass",
                    actionTitle: "Open Folder…",
                    action: { model.chooseFolder(stayOnFilesystem: false) }
                )
            } else {
                table
            }
        }
        .background(Color(nsColor: .windowBackgroundColor))
    }

    private var table: some View {
        VStack(spacing: 0) {
            HStack {
                Picker("Minimum size", selection: $model.sizeFloor) {
                    ForEach(SizeFloor.allCases) { floor in
                        Text(floor.title).tag(floor)
                    }
                }
                .accessibilityLabel("Minimum size")
                Picker("Age", selection: $model.ageFilter) {
                    ForEach(AgeFilter.allCases) { age in
                        Text(age.title).tag(age)
                    }
                }
                .accessibilityLabel("Age")
                Spacer()
                Button("Move Selected to Trash…") {
                    model.reviewPaths(Array(model.selectedLargePaths))
                }
                .disabled(model.selectedLargePaths.isEmpty)
                .accessibilityLabel("Move selected files to the Trash")
            }
            .padding(12)

            if model.largeFiles.isEmpty {
                EmptyState(
                    title: "Nothing matches",
                    message: "No file in this scan is both large enough and old enough. Lower the size or set the age to any.",
                    symbol: "line.3.horizontal.decrease.circle",
                    actionTitle: nil,
                    action: nil
                )
            } else {
                Table(model.largeFiles, selection: $model.selectedLargePaths) {
                    TableColumn("Name") { file in
                        HStack(spacing: 6) {
                            CategorySwatch(category: file.category)
                            Text(file.name).lineLimit(1)
                        }
                    }
                    TableColumn("Size") { file in
                        ByteText(bytes: file.apparentSize)
                    }
                    .width(min: 80, ideal: 100)
                    TableColumn("Modified") { file in
                        Text(modified(file.modifiedUnixMs))
                            .foregroundStyle(.secondary)
                    }
                    .width(min: 120, ideal: 160)
                    TableColumn("Kind") { file in
                        Text(file.categoryLabel)
                            .foregroundStyle(.secondary)
                    }
                    .width(min: 100, ideal: 140)
                    TableColumn("Path") { file in
                        Text(file.path)
                            .foregroundStyle(.secondary)
                            .lineLimit(1)
                    }
                }
                .contextMenu(forSelectionType: FileRow.self) { paths in
                    Button("Reveal in Finder") {
                        if let path = paths.first {
                            model.reveal(path)
                        }
                    }
                    Button("Quick Look") {
                        model.quickLook(paths: Array(paths))
                    }
                    Button("Move to Trash…") {
                        model.reviewPaths(Array(paths))
                    }
                }
            }
        }
    }

    private func modified(_ ms: Int64?) -> String {
        guard let ms else { return "—" }
        let date = Date(timeIntervalSince1970: TimeInterval(ms) / 1000)
        return date.formatted(date: .abbreviated, time: .omitted)
    }
}

extension FileRow: Identifiable {
    public var id: String { path }
}
