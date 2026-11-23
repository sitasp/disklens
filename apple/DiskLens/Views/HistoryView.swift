import SwiftUI

struct HistoryView: View {
    var model: AppModel

    var body: some View {
        Group {
            if model.history.entries.isEmpty {
                EmptyState(
                    title: "No cleanups yet",
                    message: "When you move files to the Trash, they show up here with the original path so you can put them back while they are still in the Trash.",
                    symbol: "clock.arrow.circlepath",
                    actionTitle: nil,
                    action: nil
                )
            } else {
                List(model.history.entries) { entry in
                    VStack(alignment: .leading, spacing: 6) {
                        HStack {
                            Text(entry.note)
                                .font(.headline)
                            if entry.permanent {
                                Text("Permanent")
                                    .font(.caption2.weight(.semibold))
                                    .foregroundStyle(.red)
                            }
                            if entry.undone {
                                Text("Restored")
                                    .font(.caption2.weight(.semibold))
                                    .foregroundStyle(Palette.accent)
                            }
                            Spacer()
                            ByteText(bytes: entry.byteEstimate)
                                .foregroundStyle(.secondary)
                        }
                        Text(entry.createdAt.formatted(date: .abbreviated, time: .shortened))
                            .font(.caption)
                            .foregroundStyle(.secondary)
                        ForEach(entry.items) { item in
                            Text(item.originalPath)
                                .font(.caption)
                                .foregroundStyle(.secondary)
                                .lineLimit(1)
                        }
                        if !entry.permanent && !entry.undone {
                            Button("Put Back") {
                                Task { await model.putBack(entry) }
                            }
                            .accessibilityLabel("Put files back")
                        }
                    }
                    .padding(.vertical, 4)
                }
            }
        }
        .background(Color(nsColor: .windowBackgroundColor))
        .overlay(alignment: .bottom) {
            if let error = model.history.lastError {
                Text(error)
                    .font(.caption)
                    .foregroundStyle(.red)
                    .padding(8)
            }
        }
    }
}
