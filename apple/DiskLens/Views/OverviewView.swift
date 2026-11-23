import SwiftUI

struct OverviewView: View {
    @Bindable var model: AppModel

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 20) {
                header
                if model.volumesLoading && model.volumes.isEmpty {
                    ProgressView("Reading volumes…")
                        .padding(.top, 24)
                } else if let error = model.volumesError, model.volumes.isEmpty {
                    EmptyState(
                        title: "No volumes to show",
                        message: error,
                        symbol: "externaldrive.badge.exclamationmark",
                        actionTitle: "Try Again",
                        action: { model.refreshVolumes() }
                    )
                } else {
                    LazyVGrid(columns: [GridItem(.adaptive(minimum: 280), spacing: 16)], spacing: 16) {
                        ForEach(model.volumes) { volume in
                            VolumeCard(volume: volume) {
                                model.beginScan(path: volume.path, stayOnFilesystem: true)
                                model.section = .map
                            }
                        }
                    }
                }
            }
            .padding(24)
            .frame(maxWidth: .infinity, alignment: .leading)
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
        VStack(alignment: .leading, spacing: 6) {
            Text("Where the space is")
                .font(.title2.weight(.semibold))
            Text("Pick a volume for a map that stays on that disk, or open any folder. Sizes inside a map are the files DiskLens could read. The bars below are what the volume itself reports, including snapshots and purgeable space.")
                .font(.callout)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
        }
    }
}

private struct VolumeCard: View {
    var volume: VolumeInfo
    var scan: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Image(systemName: volume.removable ? "externaldrive" : "internaldrive")
                    .foregroundStyle(Palette.accent)
                Text(volume.name)
                    .font(.headline)
                Spacer()
            }
            Text(volume.path)
                .font(.caption)
                .foregroundStyle(.secondary)
                .lineLimit(1)
            if let usage = volume.usage {
                UsageBar(used: Double(usage.usedBytes), total: Double(usage.totalBytes))
                HStack {
                    ByteText(bytes: usage.usedBytes, emphasis: true)
                    Text("used")
                        .foregroundStyle(.secondary)
                    Spacer()
                    ByteText(bytes: usage.availableBytes)
                    Text("available")
                        .foregroundStyle(.secondary)
                }
                .font(.caption)
            } else if let error = volume.error {
                Text(error)
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            Button("Scan this volume", action: scan)
                .accessibilityLabel("Scan \(volume.name)")
        }
        .padding(16)
        .background(.background, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 12, style: .continuous)
                .strokeBorder(Color.primary.opacity(0.08))
        )
    }
}
