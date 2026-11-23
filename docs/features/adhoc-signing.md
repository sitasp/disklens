# Ad-hoc signing

The Xcode project used `CODE_SIGN_STYLE: Automatic`. On a Mac with no Apple ID team selected, `xcodebuild` stops before compile with a signing error. DiskLens only has to launch on the machine that built it, so the project now signs the way Xcode's "Sign to Run Locally" does.

## Settings

`apple/project.yml` sets these on the project, so every target inherits them:

| Setting | Value | Why |
| --- | --- | --- |
| `CODE_SIGN_STYLE` | `Manual` | Automatic style asks for a development team |
| `CODE_SIGN_IDENTITY` | `"-"` | The ad-hoc identity. The quotes matter; a bare `-` is a YAML list item |
| `DEVELOPMENT_TEAM` | `""` | Explicitly empty |
| `CODE_SIGNING_REQUIRED` | `YES` | The app is still signed, so Gatekeeper and the hardened runtime have a signature to check |
| `CODE_SIGNING_ALLOWED` | `YES` | |
| `ENABLE_HARDENED_RUNTIME` | `YES` | Kept from the original spec |

`ALWAYS_SEARCH_USER_PATHS` is `NO`. Xcode 27 warns that the traditional headermap style is unsupported when that setting is left on. XcodeGen's base preset also sets it, once the presets actually load (see [xcodegen-presets.md](xcodegen-presets.md)).

The DiskLens target sets `PRODUCT_NAME` and `PRODUCT_MODULE_NAME` to `DiskLens` even though the preset would default `PRODUCT_NAME` to `$(TARGET_NAME)`. That keeps the `.app` name if a future XcodeGen run misses the presets again. The first failed build, before the preset fix, produced a product literally named `.app` and a Swift module name of `""`.

## Entitlements

The entitlement files did not change.

- `apple/DiskLens/DiskLens.entitlements` leaves the sandbox off and allows user-selected read-write.
- `apple/DiskLens/DiskLensStore.entitlements` turns the sandbox on, with user-selected read-write and app-scoped bookmarks.

For a Debug build, Xcode still injects `com.apple.security.get-task-allow`. That is what lets XCTest attach to the test host. Both schemes' unit tests launched the host and ran. The sandboxed host did not need the sandbox turned off for the existing tests, which only touch path logic, a temp history file, and in-memory gates.

`codesign -dv` on the built `DiskLens.app` reports `Signature=adhoc` and `TeamIdentifier=not set`.

## What "development team" means

It is not the DiskLens team and not the Origin namespace `dummy-inc`. Apple uses "development team" for a paid Apple Developer Program membership. That membership has a Team ID (ten characters). Xcode uses it to issue a Developer ID certificate. Notarization submits the signed app to Apple's notary service with that certificate. This Mac has no code-signing identity (`security find-identity -p codesigning` lists none), so there is nothing to notarize with. Enrolling at developer.apple.com is the step that creates the team. After that, set `DEVELOPMENT_TEAM` in `apple/project.yml` to the Team ID and switch `CODE_SIGN_IDENTITY` from `"-"` to `Developer ID Application`. Until then, local ad-hoc signing is the whole signing story.

## What this does not do

Ad-hoc signing is enough to build, test, and open the app on this Mac. It is not enough to notarize, staple a ticket, or upload to the App Store. Hardened runtime stays on so a later switch to a Developer ID certificate does not also have to rediscover which entitlements the app needs.

`xcodebuild` still prints `Metadata extraction skipped, no AppIntents.framework dependency found`. The app does not import App Intents. The message is from `appintentsmetadataprocessor` and does not fail the build.
