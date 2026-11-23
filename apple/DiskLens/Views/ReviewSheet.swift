import SwiftUI

struct ReviewSheet: View {
    @Bindable var model: AppModel
    var plan: DeletionPlanRecord

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text("Review before anything moves")
                .font(.title2.weight(.semibold))
            Text("\(plan.entries.count) items · \(formatByteCount(bytes: plan.reclaimableBytes)) estimated back if every selected copy is removed. Hard links only count once, and only when every name for that file is in this list.")
                .font(.callout)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)

            List {
                ForEach(plan.entries, id: \.path) { entry in
                    VStack(alignment: .leading, spacing: 2) {
                        HStack {
                            Text(URL(fileURLWithPath: entry.path).lastPathComponent)
                            if entry.protected {
                                Text("Protected")
                                    .font(.caption2.weight(.semibold))
                                    .foregroundStyle(.orange)
                            }
                            Spacer()
                            ByteText(bytes: entry.apparentSize)
                        }
                        Text(entry.path)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                            .lineLimit(1)
                        if let note = entry.note {
                            Text(note)
                                .font(.caption)
                                .foregroundStyle(.orange)
                        }
                        if let reason = entry.protectionReason {
                            Text(reason)
                                .font(.caption)
                                .foregroundStyle(.orange)
                        }
                    }
                }
                if !plan.skipped.isEmpty {
                    Section("Left alone") {
                        ForEach(plan.skipped, id: \.path) { skipped in
                            VStack(alignment: .leading, spacing: 2) {
                                Text(skipped.path).lineLimit(1)
                                Text(skipped.message)
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                            }
                        }
                    }
                }
            }
            .frame(minHeight: 220)

            Toggle("Delete permanently instead of using the Trash", isOn: $model.reviewPermanent)
                .accessibilityHint("Permanent deletion cannot be undone from History.")
            if model.reviewPermanent {
                Text("Type DELETE to confirm. History will record the paths, but Put Back will not be able to restore them.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                TextField("DELETE", text: $model.reviewPhrase)
                    .textFieldStyle(.roundedBorder)
                    .accessibilityLabel("Confirmation phrase")
            }
            if let error = model.reviewError {
                Text(error)
                    .font(.caption)
                    .foregroundStyle(.red)
                    .fixedSize(horizontal: false, vertical: true)
            }
            HStack {
                Button("Cancel") { model.review = nil }
                    .keyboardShortcut(.cancelAction)
                Spacer()
                if model.reviewPermanent {
                    Button("Delete Permanently") { model.confirmPermanentDelete() }
                        .keyboardShortcut(.defaultAction)
                        .disabled(!PermanentDeleteGate.accepts(model.reviewPhrase) || plan.entries.isEmpty)
                } else {
                    Button("Move to Trash") { model.confirmMoveToTrash() }
                        .keyboardShortcut(.defaultAction)
                        .disabled(plan.entries.isEmpty)
                }
            }
        }
        .padding(20)
        .frame(width: 640, height: 560)
    }
}
