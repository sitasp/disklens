import AppKit
import SwiftUI

struct DuplicatesView: View {
    @Bindable var model: AppModel

    var body: some View {
        Group {
            switch model.duplicateActivity {
            case .idle:
                EmptyState(
                    title: "Find copies, keep one",
                    message: "DiskLens groups files that are the same size, then the same at both ends, then the same all the way through. Hard links of a single file are not listed. Protected places are never checked for deletion.",
                    symbol: "square.on.square",
                    actionTitle: "Choose Folder…",
                    action: { chooseAndSearch() }
                )
            case .running:
                running
            case .failed(let message):
                EmptyState(
                    title: "Duplicate search failed",
                    message: message,
                    symbol: "exclamationmark.triangle",
                    actionTitle: "Try Again",
                    action: { chooseAndSearch() }
                )
            case .cancelled:
                EmptyState(
                    title: "Search cancelled",
                    message: "No groups were kept. Start again to compare the folder.",
                    symbol: "stop.circle",
                    actionTitle: "Choose Folder…",
                    action: { chooseAndSearch() }
                )
            case .ready:
                results
            }
        }
        .background(Color(nsColor: .windowBackgroundColor))
    }

    private var running: some View {
        VStack(spacing: 14) {
            ProgressView()
                .controlSize(.large)
            if let progress = model.duplicateProgress {
                PhaseLabel(phase: progress.phase)
                    .font(.headline)
                Text("\(progress.filesSeen) candidates · \(progress.groupsFound) groups")
                    .foregroundStyle(.secondary)
                    .monospacedDigit()
                if !progress.currentPath.isEmpty {
                    Text(progress.currentPath)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .lineLimit(2)
                        .frame(maxWidth: 520)
                }
            } else {
                Text("Starting…")
                    .foregroundStyle(.secondary)
            }
            Button("Stop") { model.cancelDuplicates() }
                .accessibilityLabel("Stop duplicate search")
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private var results: some View {
        VStack(spacing: 0) {
            HStack {
                if let root = model.duplicateRoot {
                    Text(root)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                }
                Spacer()
                Text("\(model.duplicateTotal) groups")
                    .foregroundStyle(.secondary)
                    .monospacedDigit()
                if model.unreadableCount > 0 {
                    Text("\(model.unreadableCount) unreadable")
                        .foregroundStyle(.orange)
                }
                Button("Smart Selection") { model.applySmartSelection() }
                    .accessibilityLabel("Apply smart selection")
                Button("Move Selected to Trash…") {
                    model.reviewPaths(model.selectedDuplicatePaths())
                }
                .disabled(model.selectedDuplicatePaths().isEmpty)
                .accessibilityLabel("Move selected duplicates to the Trash")
            }
            .padding(12)

            if model.groups.isEmpty {
                EmptyState(
                    title: "No duplicates",
                    message: "No two files in this folder share both a size and contents, aside from hard links of the same file.",
                    symbol: "checkmark.circle",
                    actionTitle: nil,
                    action: nil
                )
            } else {
                List {
                    ForEach(Array(model.groups.enumerated()), id: \.element.hashHex) { index, group in
                        groupSection(index: index, group: group)
                    }
                    if model.duplicateLoaded < model.duplicateTotal {
                        Button("Show more groups") { model.loadMoreDuplicates() }
                            .accessibilityLabel("Show more duplicate groups")
                    }
                }
                .listStyle(.inset)
            }
        }
    }

    @ViewBuilder
    private func groupSection(index: Int, group: DuplicateGroupRecord) -> some View {
        Section {
            if model.decisions.indices.contains(index) {
                ForEach(Array(model.decisions[index].files.enumerated()), id: \.element.path) { fileIndex, file in
                    HStack(spacing: 8) {
                        Toggle(
                            "",
                            isOn: Binding(
                                get: { file.selectedForRemoval },
                                set: { _ in model.toggleDuplicate(group: index, file: fileIndex) }
                            )
                        )
                        .labelsHidden()
                        .disabled(file.protected)
                        .accessibilityLabel(file.selectedForRemoval ? "Selected for removal" : "Keep")
                        VStack(alignment: .leading, spacing: 2) {
                            HStack {
                                Text(URL(fileURLWithPath: file.path).lastPathComponent)
                                    .lineLimit(1)
                                if file.keeper {
                                    Text("Keep")
                                        .font(.caption2.weight(.semibold))
                                        .padding(.horizontal, 6)
                                        .padding(.vertical, 2)
                                        .background(Palette.accent.opacity(0.18), in: Capsule())
                                }
                                if file.protected {
                                    Text("Protected")
                                        .font(.caption2.weight(.semibold))
                                        .padding(.horizontal, 6)
                                        .padding(.vertical, 2)
                                        .background(Color.orange.opacity(0.18), in: Capsule())
                                }
                            }
                            Text(file.explanation)
                                .font(.caption)
                                .foregroundStyle(.secondary)
                                .lineLimit(2)
                            Text(file.path)
                                .font(.caption2)
                                .foregroundStyle(.tertiary)
                                .lineLimit(1)
                        }
                        Spacer()
                        Button {
                            model.quickLook(paths: [file.path])
                        } label: {
                            Image(systemName: "eye")
                        }
                        .buttonStyle(.borderless)
                        .help("Quick Look")
                        .accessibilityLabel("Quick Look \(URL(fileURLWithPath: file.path).lastPathComponent)")
                    }
                    .padding(.vertical, 2)
                }
            }
        } header: {
            HStack {
                Text("\(group.files.count) copies")
                Text(formatByteCount(bytes: group.size))
                    .foregroundStyle(.secondary)
                Spacer()
                Text("\(formatByteCount(bytes: group.wastedBytes)) recoverable if one copy is kept")
                    .foregroundStyle(.secondary)
            }
            .font(.callout)
            .textCase(nil)
        }
    }

    private func chooseAndSearch() {
        let panel = NSOpenPanel()
        panel.canChooseFiles = false
        panel.canChooseDirectories = true
        panel.prompt = "Compare"
        panel.message = "Choose a folder to search for duplicate files."
        guard panel.runModal() == .OK, let url = panel.url else { return }
        model.beginDuplicates(path: url.path)
    }
}
