import SwiftUI

struct MapView: View {
    @Bindable var model: AppModel

    var body: some View {
        Group {
            switch model.scanActivity {
            case .idle:
                EmptyState(
                    title: "Nothing mapped yet",
                    message: "Open a folder or scan a volume. DiskLens reads names and sizes, then draws the folders that actually take the space.",
                    symbol: "circle.hexagongrid",
                    actionTitle: "Open Folder…",
                    action: { model.chooseFolder(stayOnFilesystem: false) }
                )
            case .running:
                running
            case .failed(let message):
                EmptyState(
                    title: "The scan did not finish",
                    message: message,
                    symbol: "exclamationmark.triangle",
                    actionTitle: "Try Again",
                    action: {
                        if let root = model.scanRoot {
                            model.beginScan(path: root, stayOnFilesystem: false)
                        } else {
                            model.chooseFolder(stayOnFilesystem: false)
                        }
                    }
                )
            case .cancelled:
                EmptyState(
                    title: "Scan cancelled",
                    message: "Nothing was kept from the partial walk. Start again when you want the full map.",
                    symbol: "stop.circle",
                    actionTitle: "Open Folder…",
                    action: { model.chooseFolder(stayOnFilesystem: false) }
                )
            case .ready:
                mapped
            }
        }
        .background(Color(nsColor: .windowBackgroundColor))
    }

    private var running: some View {
        ScanActivityPanel(
            phase: model.scanProgress?.phase,
            stats: model.scanProgress.map {
                "\($0.filesSeen) entries · \(formatByteCount(bytes: $0.bytesSeen)) seen"
            } ?? "Starting the walk…",
            path: model.scanProgress?.currentPath ?? "",
            stop: { model.cancelScan() },
            stopLabel: "Stop scan"
        )
    }

    private var mapped: some View {
        VStack(spacing: 0) {
            breadcrumb
            ViewThatFits(in: .horizontal) {
                HStack(spacing: 0) {
                    chart.frame(minWidth: 420)
                    Divider()
                    outline.frame(minWidth: 280, idealWidth: 340, maxWidth: 460)
                }
                VStack(spacing: 0) {
                    chart.frame(minHeight: 320)
                    Divider()
                    outline.frame(minHeight: 220)
                }
            }
            inspector
        }
    }

    private var breadcrumb: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 4) {
                ForEach(Array(model.crumbs.enumerated()), id: \.offset) { index, crumb in
                    if index > 0 {
                        Image(systemName: "chevron.right")
                            .font(.caption2)
                            .foregroundStyle(.tertiary)
                    }
                    Button(crumb.title) {
                        model.drill(to: crumb.path)
                    }
                    .buttonStyle(.plain)
                    .font(index == model.crumbs.count - 1 ? .callout.weight(.semibold) : .callout)
                    .accessibilityLabel("Open \(crumb.title)")
                }
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 10)
        }
        .background(.bar)
    }

    private var chart: some View {
        VStack(spacing: 8) {
            Picker("Visualization", selection: $model.visualization) {
                ForEach(MapVisualization.allCases) { mode in
                    Text(mode.title).tag(mode)
                }
            }
            .pickerStyle(.segmented)
            .frame(maxWidth: 280)
            .padding(.top, 12)
            .accessibilityLabel("Map style")

            Group {
                if model.visualization == .sunburst {
                    SunburstCanvas(
                        arcs: model.arcs,
                        title: DiskPaths.displayName(model.currentPath ?? ""),
                        onHover: { arc in
                            guard let arc, !arc.path.isEmpty, let node = model.session?.node(path: arc.path) else {
                                model.hovered = nil
                                return
                            }
                            model.hovered = node
                        },
                        onSelect: { arc in
                            if arc.kind == .directory {
                                model.drill(to: arc.path)
                            }
                        },
                        onCenter: { model.goUp() }
                    )
                } else {
                    TreemapCanvas(
                        rects: model.rects,
                        onHover: { rect in
                            guard let rect, let node = model.session?.node(path: rect.path) else {
                                model.hovered = nil
                                return
                            }
                            model.hovered = node
                        },
                        onSelect: { rect in
                            if rect.kind == .directory {
                                model.drill(to: rect.path)
                            }
                        },
                        onResize: { width, height in
                            model.updateTreemap(width: width, height: height)
                        }
                    )
                }
            }
            .padding(12)

            legend
                .padding(.bottom, 8)
        }
    }

    private var legend: some View {
        let present = Array(Set(model.children.map(\.category))).sorted { $0.label < $1.label }
        return HStack(spacing: 12) {
            ForEach(present, id: \.self) { category in
                HStack(spacing: 4) {
                    CategorySwatch(category: category)
                    Text(category.label)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }
        }
        .frame(maxWidth: .infinity)
    }

    private var outline: some View {
        VStack(spacing: 0) {
            Picker("Sort", selection: $model.outlineSort) {
                ForEach(OutlineSort.allCases) { sort in
                    Text(sort.title).tag(sort)
                }
            }
            .pickerStyle(.segmented)
            .padding(12)
            .accessibilityLabel("Sort files")

            List(model.displayedChildren, id: \.path) { node in
                Button {
                    if node.kind == .directory {
                        model.drill(to: node.path)
                    } else {
                        model.hovered = node
                    }
                } label: {
                    HStack(spacing: 8) {
                        CategorySwatch(category: node.category)
                        Image(systemName: symbol(for: node.kind))
                            .foregroundStyle(.secondary)
                            .frame(width: 16)
                        VStack(alignment: .leading, spacing: 2) {
                            Text(node.name)
                                .lineLimit(1)
                            if let error = node.error {
                                Text(error)
                                    .font(.caption2)
                                    .foregroundStyle(.orange)
                                    .lineLimit(1)
                            }
                        }
                        Spacer(minLength: 8)
                        ByteText(bytes: size(of: node))
                            .font(.callout)
                            .foregroundStyle(.secondary)
                    }
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .accessibilityLabel("\(node.name), \(formatByteCount(bytes: size(of: node)))")
            }
            .listStyle(.inset)
        }
    }

    private var inspector: some View {
        HStack {
            if let node = model.hovered {
                CategorySwatch(category: node.category)
                Text(node.name).fontWeight(.medium)
                Text(node.categoryLabel)
                    .foregroundStyle(.secondary)
                if node.sharedElsewhere {
                    Text("Shared link")
                        .foregroundStyle(.secondary)
                }
                if node.otherVolume {
                    Text("Other volume")
                        .foregroundStyle(.secondary)
                }
                Spacer()
                ByteText(bytes: size(of: node), emphasis: true)
                Text("\(node.fileCount) files")
                    .foregroundStyle(.secondary)
                    .monospacedDigit()
            } else if let session = model.session, let current = model.currentPath, let node = session.node(path: current) {
                Text(node.name).fontWeight(.medium)
                Text("\(node.fileCount) files")
                    .foregroundStyle(.secondary)
                if session.issueCount() > 0 {
                    Text("\(session.issueCount()) unreadable")
                        .foregroundStyle(.orange)
                }
                Spacer()
                ByteText(bytes: size(of: node), emphasis: true)
                Text(formatByteCount(bytes: node.logicalApparent) + " logical")
                    .foregroundStyle(.secondary)
                    .monospacedDigit()
            } else {
                Spacer()
            }
        }
        .font(.callout)
        .padding(.horizontal, 16)
        .padding(.vertical, 8)
        .background(.bar)
    }

    private func size(of node: NodeSummary) -> UInt64 {
        switch model.metric {
        case .uniqueApparent: return node.uniqueApparent
        case .uniqueAllocated: return node.uniqueAllocated
        case .logicalApparent: return node.logicalApparent
        }
    }

    private func symbol(for kind: NodeKind) -> String {
        switch kind {
        case .directory: return "folder"
        case .file: return "doc"
        case .symlink: return "link"
        case .other: return "questionmark.square"
        }
    }
}

private extension FileCategory {
    var label: String {
        switch self {
        case .application: return "Applications"
        case .audio: return "Audio"
        case .video: return "Video"
        case .image: return "Images"
        case .document: return "Documents"
        case .archive: return "Archives"
        case .code: return "Code"
        case .developerCache: return "Developer caches"
        case .system: return "System"
        case .other: return "Other"
        }
    }
}
