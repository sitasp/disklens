# Open panel and system locations

The open panel remembers the last directory it confirmed. During a check on this Mac that directory was `/Applications`. The next confirmation, including a bare Return, started a read-only walk of every app. Nothing was deleted. The walk itself was the problem: hundreds of thousands of files, and not the folder anyone meant to map.

## Where the panel opens

`AppModel.openFolderPanel` sets `directoryURL` before `runModal`.

- A caller that passes `startingAt` still opens there. The sandboxed volume card does this so the panel is already pointed at that volume.
- Every other open starts at `FileManager.default.homeDirectoryForCurrentUser`. That replaces the remembered directory, so a later Open Folder does not land back in `/Applications`.

## A second confirmation

`DiskPaths.isBroadSystemLocation` is true only for the location itself, after `standardizedFileURL`:

`/`, `/Applications`, `/System`, `/Library`, `/Users`, `/Volumes`, `/usr`, `/bin`, `/sbin`, `/opt`, `/private`.

`/Applications/Xcode.app` is not on the list. `/Applications/` is the same path as `/Applications`. `/Applications-old` does not match, because the check is equality, not a prefix.

`chooseFolder` and `chooseDuplicateFolder` call `confirmIfBroadSystemLocation` after the panel returns and before access is granted or the walk starts. The alert names the path, says the walk only reads, and offers the action ("Scan" or "Compare") or "Choose Again". Choose Again reopens the panel. Dismissing the panel stops without calling `beginScan` or `beginDuplicates`.

The Overview button "Scan this volume" does not go through the panel. It still calls `beginScan` on that volume directly, because the button already says what will be scanned.

## Test

`testBroadSystemLocationsAreExact` in `PathAndHistoryTests` covers `/Applications`, the trailing slash, `/`, a folder inside Applications, a home folder, and `/Applications-old`.
