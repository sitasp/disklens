import SwiftUI

struct RootView: View {
    @Bindable var model: AppModel

    var body: some View {
        NavigationSplitView {
            List(selection: $model.section) {
                Section("Library") {
                    ForEach(SidebarSection.allCases) { section in
                        Label(section.title, systemImage: section.symbol)
                            .tag(section)
                    }
                }
            }
            .listStyle(.sidebar)
            .navigationSplitViewColumnWidth(min: 196, ideal: 216, max: 260)
            .navigationTitle("DiskLens")
        } detail: {
            detail
                .navigationTitle(model.section.title)
                .toolbar { toolbar }
        }
        .sheet(isPresented: $model.showOnboarding) {
            OnboardingView(model: model)
        }
        .sheet(item: reviewBinding) { plan in
            ReviewSheet(model: model, plan: plan)
        }
        .alert(
            model.protectedPrompt?.title ?? "Protected location",
            isPresented: protectedPromptPresented
        ) {
            Button("Select Anyway") { model.confirmProtectedPrompt() }
            Button("Leave It", role: .cancel) { model.protectedPrompt = nil }
        } message: {
            Text(model.protectedPrompt?.message ?? "")
        }
        .task {
            if model.volumes.isEmpty {
                model.refreshVolumes()
            }
        }
    }

    @ViewBuilder
    private var detail: some View {
        switch model.section {
        case .overview:
            OverviewView(model: model)
        case .map:
            MapView(model: model)
        case .categories:
            CategoryView(model: model)
        case .largeFiles:
            LargeFilesView(model: model)
        case .duplicates:
            DuplicatesView(model: model)
        case .history:
            HistoryView(model: model)
        }
    }

    @ToolbarContentBuilder
    private var toolbar: some ToolbarContent {
        ToolbarItemGroup {
            Button {
                model.chooseFolder(stayOnFilesystem: false)
            } label: {
                Label("Open Folder", systemImage: "folder")
            }
            .help("Choose a folder to scan")
            .accessibilityLabel("Open folder")

            if model.section == .map, model.scanActivity == .running {
                Button {
                    model.cancelScan()
                } label: {
                    Label("Stop", systemImage: "stop.fill")
                }
                .accessibilityLabel("Stop scan")
            }
            if model.section == .duplicates, model.duplicateActivity == .running {
                Button {
                    model.cancelDuplicates()
                } label: {
                    Label("Stop", systemImage: "stop.fill")
                }
                .accessibilityLabel("Stop duplicate search")
            }
        }
    }

    private var protectedPromptPresented: Binding<Bool> {
        Binding(
            get: { model.protectedPrompt != nil },
            set: { if !$0 { model.protectedPrompt = nil } }
        )
    }

    private var reviewBinding: Binding<DeletionPlanRecord?> {
        Binding(
            get: { model.review },
            set: { model.review = $0 }
        )
    }
}

extension DeletionPlanRecord: Identifiable {
    public var id: String {
        entries.map(\.path).joined(separator: "\n") + "|\(reclaimableBytes)"
    }
}
