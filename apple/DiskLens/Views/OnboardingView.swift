import SwiftUI

struct OnboardingView: View {
    var model: AppModel

    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            Text(Distribution.isAppStore ? "Pick the folders DiskLens may read" : "DiskLens needs room to look")
                .font(.title2.weight(.semibold))
            if Distribution.isAppStore {
                Text("This is the App Store build. It only reads folders you choose. DiskLens saves a security-scoped bookmark for each one, uses that bookmark on the next scan, and asks macOS for access again before it deletes anything.")
                    .fixedSize(horizontal: false, vertical: true)
                VStack(alignment: .leading, spacing: 8) {
                    step(1, "Choose a folder from the open panel. That choice is the permission.")
                    step(2, "Rescans of that folder reuse the saved bookmark. If macOS expires it, you will be asked to pick the folder again.")
                    step(3, "Before a deletion, DiskLens confirms the bookmark still opens. If it does not, grant access again from the review screen.")
                }
            } else {
                Text("A folder you pick in the open panel can be scanned without extra permission. Full Disk Access is what lets the map include Mail, Messages, and other protected folders. macOS does not grant it from inside the app. You turn it on once.")
                    .fixedSize(horizontal: false, vertical: true)
                VStack(alignment: .leading, spacing: 8) {
                    step(1, "Open System Settings → Privacy & Security → Full Disk Access.")
                    step(2, "Unlock the pane, then add DiskLens. If it is missing, use the add button and choose it from Applications.")
                    step(3, "Quit and reopen DiskLens so the new permission applies.")
                }
                Text("This build is not sandboxed, so it can map a whole volume. The App Store build is a separate target that only sees folders you pick. See the sandbox notes in the repository.")
                    .font(.callout)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
            HStack {
                if !Distribution.isAppStore {
                    Button("Open Full Disk Access") { model.openFullDiskAccessSettings() }
                }
                Spacer()
                Button("Continue") { model.dismissOnboarding() }
                    .keyboardShortcut(.defaultAction)
            }
        }
        .padding(24)
        .frame(width: 560)
    }

    private func step(_ number: Int, _ text: String) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: 10) {
            Text("\(number)")
                .font(.caption.weight(.bold))
                .frame(width: 20, height: 20)
                .background(Palette.accent.opacity(0.2), in: Circle())
            Text(text)
                .fixedSize(horizontal: false, vertical: true)
        }
    }
}
