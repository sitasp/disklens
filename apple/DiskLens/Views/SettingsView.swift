import SwiftUI

struct SettingsView: View {
    @Bindable var model: AppModel

    var body: some View {
        @Bindable var settings = model.settings
        Form {
            Section("Duplicate selection") {
                Picker("Keep", selection: $settings.keepStrategy) {
                    Text("Newest").tag(KeepStrategy.newest)
                    Text("Oldest").tag(KeepStrategy.oldest)
                    Text("Preferred folder").tag(KeepStrategy.preferredDirectory)
                    Text("Shortest path").tag(KeepStrategy.shortestPath)
                    Text("Original name").tag(KeepStrategy.mostOriginalName)
                }
                TextField("Preferred folder", text: $settings.preferredDirectory)
                    .accessibilityLabel("Preferred folder")
                Text("If the preferred folder does not contain a copy, DiskLens keeps the shortest path.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            Section("Never auto-select") {
                Toggle("System locations", isOn: $settings.protectSystem)
                Toggle("Library folders", isOn: $settings.protectLibrary)
                Toggle("App bundles and photo libraries", isOn: $settings.protectPackages)
                Text("These files can still be removed if you select them yourself and confirm the review.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            Section("Scanning") {
                TextField(
                    "Ignore duplicates smaller than (bytes)",
                    value: $settings.minimumDuplicateSize,
                    format: .number
                )
                Toggle("Follow symbolic links", isOn: $settings.followSymlinks)
                Toggle("Include hidden files", isOn: $settings.includeHidden)
                Toggle("Detect APFS clones", isOn: $settings.detectClones)
                Text("Clone detection asks macOS for a clone id on every file. Turn it off for a faster scan that may count cloned files twice.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            Section("Full Disk Access") {
                Button("Show the access guide") { model.showOnboarding = true }
            }
        }
        .formStyle(.grouped)
        .padding(8)
    }
}
