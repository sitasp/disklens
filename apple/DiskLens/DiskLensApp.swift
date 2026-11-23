import SwiftUI

@main
struct DiskLensApp: App {
    @State private var model = AppModel()

    var body: some Scene {
        WindowGroup {
            RootView(model: model)
                .frame(minWidth: 980, minHeight: 680)
        }
        .defaultSize(width: 1180, height: 760)
        .commands {
            CommandGroup(replacing: .newItem) {
                Button("Open Folder…") {
                    model.chooseFolder(stayOnFilesystem: false)
                }
                .keyboardShortcut("o")
                Button("Rescan") {
                    if let root = model.scanRoot {
                        model.beginScan(path: root, stayOnFilesystem: false)
                    }
                }
                .keyboardShortcut("r")
                .disabled(model.scanRoot == nil)
            }
            CommandMenu("Map") {
                Button("Cancel Scan") { model.cancelScan() }
                    .keyboardShortcut(".", modifiers: .command)
                    .disabled(model.scanActivity != .running)
                Button("Sunburst") { model.visualization = .sunburst }
                    .keyboardShortcut("1")
                Button("Treemap") { model.visualization = .treemap }
                    .keyboardShortcut("2")
                Button("Enclosing Folder") { model.goUp() }
                    .keyboardShortcut(.upArrow, modifiers: .command)
                Divider()
                Picker("Size", selection: $model.metric) {
                    Text("Unique").tag(SizeMetric.uniqueApparent)
                    Text("Allocated").tag(SizeMetric.uniqueAllocated)
                    Text("Logical").tag(SizeMetric.logicalApparent)
                }
            }
            CommandMenu("Cleanup") {
                Button("Move Selected to Trash…") {
                    let paths = model.section == .duplicates
                        ? model.selectedDuplicatePaths()
                        : Array(model.selectedLargePaths)
                    model.reviewPaths(paths)
                }
                .keyboardShortcut(.delete, modifiers: .command)
                Button("Quick Look") {
                    let paths = model.section == .duplicates
                        ? model.selectedDuplicatePaths()
                        : Array(model.selectedLargePaths)
                    model.quickLook(paths: paths)
                }
                .keyboardShortcut(.space, modifiers: [])
            }
        }

        Settings {
            SettingsView(model: model)
                .frame(width: 520, height: 460)
        }
    }
}
