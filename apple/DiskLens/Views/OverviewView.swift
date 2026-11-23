import SwiftUI

struct OverviewView: View {
    @Bindable var model: AppModel

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 28) {
                header
                if model.volumesLoading && model.volumes.isEmpty {
                    ProgressView("Reading volumes…")
                        .font(.title3)
                        .padding(.top, 12)
                } else if let error = model.volumesError, model.volumes.isEmpty {
                    EmptyState(
                        title: "No volumes to show",
                        message: error,
                        symbol: "externaldrive.badge.exclamationmark",
                        actionTitle: "Try Again",
                        action: { model.refreshVolumes() }
                    )
                    .frame(minHeight: 320)
                } else if let volume = DashboardVolumes.featured(model.volumes) {
                    hero(volume)
                }
                actions
                if model.session != nil {
                    lastScan
                }
            }
            .padding(32)
            .frame(maxWidth: 1080, alignment: .leading)
        }
        .background(Color(nsColor: .windowBackgroundColor))
        .toolbar {
            ToolbarItem {
                Button {
                    model.refreshVolumes()
                } label: {
                    Label("Refresh", systemImage: "arrow.clockwise")
                }
                .accessibilityLabel("Refresh volumes")
            }
        }
    }

    private var header: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Where the space is")
                .font(.largeTitle.weight(.semibold))
            Text("One disk, the last map, and the next thing worth opening. The big number is what the volume reports. A map counts the files DiskLens could read.")
                .font(.title3)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
        }
    }

    private func hero(_ volume: VolumeInfo) -> some View {
        VStack(alignment: .leading, spacing: 18) {
            HStack(spacing: 14) {
                Image(systemName: volume.removable ? "externaldrive.fill" : "internaldrive.fill")
                    .font(.title2.weight(.semibold))
                    .foregroundStyle(Palette.accent)
                    .frame(width: 52, height: 52)
                    .background(Palette.accent.opacity(0.16), in: Circle())
                VStack(alignment: .leading, spacing: 2) {
                    Text(volume.name)
                        .font(.title2.weight(.semibold))
                    Text(volume.path)
                        .font(.body)
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                }
                Spacer()
                Button(Distribution.isAppStore ? "Choose a folder" : "Map this disk") {
                    scan(volume)
                }
                .buttonStyle(.borderedProminent)
                .controlSize(.large)
                .accessibilityLabel(Distribution.isAppStore ? "Choose a folder on \(volume.name)" : "Map \(volume.name)")
            }
            if let usage = volume.usage {
                Text(formatByteCount(bytes: usage.availableBytes))
                    .font(.system(size: 48, weight: .semibold, design: .rounded))
                    .monospacedDigit()
                Text("available of \(formatByteCount(bytes: usage.totalBytes))")
                    .font(.title3)
                    .foregroundStyle(.secondary)
                UsageBar(used: Double(usage.usedBytes), total: Double(usage.totalBytes))
                    .frame(height: 14)
                HStack(spacing: 20) {
                    stat("Used", bytes: usage.usedBytes)
                    stat("Free", bytes: usage.freeBytes)
                }
                .font(.title3)
            } else if let error = volume.error {
                Text(error)
                    .font(.body)
                    .foregroundStyle(.secondary)
            }
        }
        .padding(24)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(.background, in: RoundedRectangle(cornerRadius: 18, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 18, style: .continuous)
                .strokeBorder(Color.primary.opacity(0.08))
        )
    }

    private func stat(_ title: String, bytes: UInt64) -> some View {
        HStack(spacing: 6) {
            Text(title)
                .foregroundStyle(.secondary)
            ByteText(bytes: bytes, emphasis: true)
        }
    }

    private var actions: some View {
        LazyVGrid(columns: [GridItem(.adaptive(minimum: 220), spacing: 16)], spacing: 16) {
            ActionCard(
                title: "Disk map",
                message: "Sunburst or treemap of a folder.",
                symbol: "chart.pie.fill",
                tint: Palette.accent
            ) { model.section = .map }
            ActionCard(
                title: "Duplicates",
                message: "Same bytes, keep one copy.",
                symbol: "square.on.square.fill",
                tint: Palette.color(for: .developerCache)
            ) { model.section = .duplicates }
            ActionCard(
                title: "Large files",
                message: "The biggest and the oldest.",
                symbol: "doc.fill",
                tint: Palette.color(for: .document)
            ) { model.section = .largeFiles }
            ActionCard(
                title: "Categories",
                message: "Photos, caches, archives.",
                symbol: "square.stack.3d.up.fill",
                tint: Palette.color(for: .image)
            ) { model.section = .categories }
        }
    }

    private var lastScan: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Last map")
                .font(.title2.weight(.semibold))
            if let session = model.session, let root = model.scanRoot {
                let node = session.root()
                HStack(alignment: .firstTextBaseline) {
                    VStack(alignment: .leading, spacing: 4) {
                        Text(DiskPaths.displayName(root))
                            .font(.title3.weight(.semibold))
                        Text("\(node.fileCount) files")
                            .font(.title3)
                            .foregroundStyle(.secondary)
                    }
                    Spacer()
                    Text(formatByteCount(bytes: node.uniqueApparent))
                        .font(.title.weight(.semibold))
                        .monospacedDigit()
                }
                Button("Open the map") { model.section = .map }
                    .controlSize(.large)
            }
        }
        .padding(24)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(.background, in: RoundedRectangle(cornerRadius: 18, style: .continuous))
    }

    private func scan(_ volume: VolumeInfo) {
        if Distribution.isAppStore {
            model.chooseFolder(stayOnFilesystem: true, startingAt: volume.path)
        } else {
            model.beginScan(path: volume.path, stayOnFilesystem: true)
            model.section = .map
        }
    }
}

private struct ActionCard: View {
    var title: String
    var message: String
    var symbol: String
    var tint: Color
    var open: () -> Void

    var body: some View {
        Button(action: open) {
            VStack(alignment: .leading, spacing: 14) {
                Image(systemName: symbol)
                    .font(.title2.weight(.semibold))
                    .foregroundStyle(tint)
                    .frame(width: 52, height: 52)
                    .background(tint.opacity(0.16), in: Circle())
                Text(title)
                    .font(.title3.weight(.semibold))
                    .foregroundStyle(.primary)
                Text(message)
                    .font(.body)
                    .foregroundStyle(.secondary)
                    .multilineTextAlignment(.leading)
            }
            .padding(20)
            .frame(maxWidth: .infinity, minHeight: 168, alignment: .topLeading)
            .background(.background, in: RoundedRectangle(cornerRadius: 18, style: .continuous))
            .overlay(
                RoundedRectangle(cornerRadius: 18, style: .continuous)
                    .strokeBorder(Color.primary.opacity(0.08))
            )
        }
        .buttonStyle(.plain)
        .accessibilityLabel(title)
    }
}
