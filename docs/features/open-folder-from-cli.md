# Open a folder from the command line

The window normally starts a scan from `NSOpenPanel` (`AppModel.chooseFolder`) or from a volume card. That is the right path for a person sitting at the Mac. A launch that already knows the directory should not have to drive the open panel.

## Launch argument

`RootView` calls `model.openInitialPathIfPresent()` from its `.task`, after the volume list is refreshed if it is empty.

```swift
func openInitialPathIfPresent() {
    guard !didConsumeLaunchPath else { return }
    didConsumeLaunchPath = true
    let path = CommandLine.arguments.dropFirst().first { candidate in
        var isDirectory: ObjCBool = false
        return FileManager.default.fileExists(atPath: candidate, isDirectory: &isDirectory)
            && isDirectory.boolValue
    }
    guard let path else { return }
    dismissOnboarding()
    beginScan(path: path, stayOnFilesystem: false)
    section = .map
}
```

`didConsumeLaunchPath` makes the call one-shot. SwiftUI can re-run `.task`. The first existing directory among the arguments wins. Anything that is not a directory is ignored, including the executable path in `arguments[0]`.

When a directory is found:

- onboarding is dismissed and `didFinishOnboarding` is stored, so the sheet does not cover the map
- `beginScan` uses the same options, cancel flag, and progress relay as Open Folder
- `stayOnFilesystem` is false, matching Open Folder rather than a volume scan
- the sidebar selection moves to Disk Map

Usage, once `xcodebuild` has produced the app:

```bash
open /path/to/DiskLens.app --args /path/to/folder
```

`open` only forwards `--args` to a new process. If DiskLens is already running, quit it first.

The sandboxed target still has to start security-scoped access before a scan. This path does not present an open panel, so it does not grant a bookmark. It is for the unsandboxed DiskLens app.

## Search the folder that was just scanned

Duplicates used to be a separate open-panel flow even after a scan. `DuplicatesView`'s idle state now looks at `model.scanRoot`.

- If a scan root exists, the button is "Search \(display name)" and calls `beginDuplicates(path: root)`.
- If nothing has been scanned, the button stays "Choose Folder…" and still calls `chooseDuplicateFolder()`.

`beginDuplicates` is the same function the panel uses: it checks `folderAccess.prepareToRead`, then runs `findDuplicateFiles` off the main actor. Protected paths are still left unselected by `applySelection`. Selecting one by hand still raises the protected prompt, and the review still lists those paths in their own section until "Remove these protected items" is on.

## What was checked

A temp directory with three identical 250 KB files, one of them under a `Library` component, was passed with `--args`. The map showed 1.8 MB and 8 files. Duplicates reported one group. The `Library` copy was marked Protected and left unchecked. The review sheet's "Protected locations" section named that file and the reason "inside a Library folder", and Move to Trash stayed disabled.
