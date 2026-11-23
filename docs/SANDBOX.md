# Sandbox, bookmarks, and Full Disk Access

DiskLens has two Mac targets. They share one bundle id, `com.disklens.DiskLens`, and one source tree. They are alternate products, not two apps you install side by side.

| Scheme | Entitlements | Who it is for |
| --- | --- | --- |
| `DiskLens` | `apple/DiskLens/DiskLens.entitlements` — sandbox off, hardened runtime on | Direct distribution. Can map a volume and, with Full Disk Access, protected folders. |
| `DiskLensStore` | `apple/DiskLens/DiskLensStore.entitlements` — App Sandbox, user-selected read-write, app-scoped bookmarks | The future Mac App Store build. `PRODUCT_NAME` is `DiskLensStore` so a local build does not overwrite the direct `.app`. The display name stays DiskLens. |

`DiskLensStore` compiles with `DISKLENS_SANDBOX`. `Distribution.isAppStore` is that flag.

## Direct build

There is no Info.plist key that grants Full Disk Access. The user turns it on:

1. System Settings → Privacy & Security → Full Disk Access.
2. Add DiskLens.
3. Quit and reopen the app.

The first-launch sheet says this and tries to open the pane. Without Full Disk Access the app still scans folders chosen in the open panel. Protected folders come back as unreadable entries on the tree rather than as a failed scan.

A folder chosen in the panel is also saved as a security-scoped bookmark. The direct build does not need that bookmark to read the path.

## App Store build

The sandbox cannot see the whole disk. Every scan starts from an `NSOpenPanel`. The URL from that panel is security-scoped:

1. `startAccessingSecurityScopedResource()` runs before the scan.
2. `URL.bookmarkData(options: .withSecurityScope)` is stored in Application Support (`bookmarks.json`) so a rescan does not ask again.
3. The next scan resolves the bookmark, refreshes it if macOS marks it stale, and starts access again.
4. Before Trash or permanent delete, DiskLens resolves the bookmark that covers each path and starts access again. If that fails, the review stays open, explains that access expired, and offers **Grant Access Again…**, which is another open panel.

Volume cards in this build do not walk the volume root. They open the panel already pointed at that volume so the user picks a folder on it.

`FolderAccess.bestCovering` chooses the longest saved folder that contains a path, on a path boundary. `/Users/ada/Documents` does not cover `/Users/ada/Documents-old`.

## Protected locations and deletion

Smart selection never checks a file in a protected location (System, Library including `~/Library/Caches`, app bundles, Photos libraries, and any extra prefix). Those files stay visible.

If you select one yourself:

- A confirmation explains why it is protected and that smart rules skipped it.
- The deletion plan keeps the path and sets `protected` plus `protection_reason`. It is not moved to the skipped list.
- The review screen lists those items in their own section. **Move to Trash** and **Delete Permanently** stay disabled until **Remove these protected items** is on.
- Permanent delete still requires the word `DELETE`.

`plan_deletion(..., include_protected: false)` is the automatic path and still records protected paths as skipped. The app passes `include_protected: true` for a review of a manual selection.

## Deletion mechanics

- The default action is `FileManager.trashItem`. History stores the original path and the Trash URL.
- Permanent deletion calls `removeItem` only after `DELETE`. History records the paths and cannot put them back.
- Symlinks are not followed. The plan says only the link would be removed.
