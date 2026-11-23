# Sandbox and Full Disk Access

DiskLens ships with the App Sandbox turned off.

```xml
<key>com.apple.security.app-sandbox</key>
<false/>
```

A disk map has to read whatever folder the user points at, and with Full Disk Access it also reads locations macOS otherwise hides (Mail, Messages, other home folders, some Library subtrees). The sandbox can grant a security-scoped bookmark for a single folder chosen in an open panel. It cannot express "show me this disk." DaisyDisk, GrandPerspective, and the other apps in this category are distributed outside the Mac App Store for the same reason.

Hardened runtime is on. There is no JIT entitlement and no unsigned-memory entitlement. The Rust code is a static library, not a plugin.

## Full Disk Access

There is no Info.plist key that grants Full Disk Access. The user turns it on:

1. System Settings → Privacy & Security → Full Disk Access.
2. Add DiskLens.
3. Quit and reopen the app.

The first-launch sheet says this and tries to open the pane with the system settings URL. If that URL fails on a future macOS, the written steps still work.

Without Full Disk Access the app still scans folders the user chooses. Protected folders come back as unreadable entries on the tree rather than as a failed scan.

## What deletion is allowed to do

- The default action is `FileManager.trashItem`. The history log stores the original path and the Trash URL.
- Permanent deletion is a second control. The review sheet requires the word `DELETE` before it calls `removeItem`. History records the paths and marks them unrestorable.
- Symlinks are not followed. The plan reports that only the link would be removed.
- Smart selection never checks a protected path. The review can still include one if the user selected it themselves.

## If you later want the Mac App Store

You would have to sandbox the app, drop whole-disk scans, and limit the product to folders the user picks (security-scoped bookmarks, persisted). That is a different product. The Rust core would stay; the Swift deletion and volume list would be the parts that change.
