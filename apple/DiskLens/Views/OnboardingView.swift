import SwiftUI

struct OnboardingView: View {
    var model: AppModel

    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            Text("DiskLens needs room to look")
                .font(.title2.weight(.semibold))
            Text("A folder you pick in the open panel can be scanned without extra permission. Full Disk Access is what lets the map include Mail, Messages, and other protected folders. macOS does not grant it from inside the app. You turn it on once.")
                .fixedSize(horizontal: false, vertical: true)
            VStack(alignment: .leading, spacing: 8) {
                step(1, "Open System Settings → Privacy & Security → Full Disk Access.")
                step(2, "Unlock the pane, then add DiskLens. If it is missing, use the add button and choose it from Applications.")
                step(3, "Quit and reopen DiskLens so the new permission applies.")
            }
            Text("The app is not sandboxed. A disk utility that only sees one bookmarked folder cannot show the rest of the disk. See the sandbox notes in the repository if you plan to ship a signed build.")
                .font(.callout)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
            HStack {
                Button("Open Full Disk Access") { model.openFullDiskAccessSettings() }
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
