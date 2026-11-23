# Release versioning

DiskLens uses semantic versions. The app bundle can only show a three-number version. The alpha or beta marker lives in the git tag, the GitHub release, and a channel string in Settings.

## Numbers

| Piece | Where it lives | Rule |
| --- | --- | --- |
| Marketing version | `MARKETING_VERSION` in `apple/project.yml`, shown as `CFBundleShortVersionString` | `MAJOR.MINOR.PATCH`, three integers. Finder and macOS require this shape. |
| Build number | `CURRENT_PROJECT_VERSION`, shown as `CFBundleVersion` | Integer. Increase by one for every zip that is published. |
| Channel | `DISKLENS_RELEASE_CHANNEL` | `alpha`, `beta`, or `stable`. Settings hides `stable`. |
| Git tag | `vMAJOR.MINOR.PATCH` or `vMAJOR.MINOR.PATCH-alpha.N` / `-beta.N` | The name of the zip and the GitHub release. |
| GitHub | Release on `sitasp/disklens` | Alpha and beta are marked prerelease. |

`0.y.z` means the app is not stable yet. `1.0.0` is the first version offered without an alpha or beta label.

## How a line moves

1. `v0.1.0-alpha.1` is the first public build. Marketing version `0.1.0`, build `1`, channel `alpha`.
2. The next alpha on the same feature line is `v0.1.0-alpha.2` with build `2`. The short version stays `0.1.0`.
3. A larger cut during alpha, such as a scan or deletion change people must relearn, bumps minor: `v0.2.0-alpha.1`, build incremented again.
4. Beta starts at `v0.x.0-beta.1` with channel `beta` when the map, duplicates, and review flow are complete enough to hand to someone else without sitting with them.
5. `v1.0.0`, channel `stable`, build incremented, is the first release that is not a prerelease. Notarization can arrive before or after that tag. It does not define the number.

## What this alpha contains

The direct app only. `DiskLensStore` stays a local scheme until there is an Apple Developer Program membership to upload it.

- Folder and volume maps, sunburst and treemap
- Categories, large files, duplicates, protected-path review, Trash, history
- Ad-hoc signature. Gatekeeper will refuse a double-click. The first open is Finder, shortcut menu, Open, then Open.

## Cutting a release

From a clean `main`, with the three version fields updated and committed:

```bash
scripts/package-mac.sh v0.1.0-alpha.1
git tag v0.1.0-alpha.1
git push origin main v0.1.0-alpha.1
git push github main v0.1.0-alpha.1
gh release create v0.1.0-alpha.1 build/dist/DiskLens-v0.1.0-alpha.1-mac.zip \
  --repo sitasp/disklens --prerelease \
  --title "DiskLens 0.1.0 alpha 1"
```

`scripts/package-mac.sh` regenerates the Xcode project, builds the Release configuration of the DiskLens scheme, and writes `build/dist/DiskLens-<tag>-mac.zip`. `build/` is gitignored. The zip contains `DiskLens.app` at the top level.

Origin remains the source repo. The GitHub release is the download. Push the tag to both.
