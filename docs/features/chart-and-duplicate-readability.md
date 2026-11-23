# Chart labels and the duplicate toolbar

These are the layout changes made after running the app on a temp folder in dark mode. The scan, the sizes, and the selection rules were already correct. The drawing was not, once names got long and the category colors were the mid-tone palette.

## Sunburst center

`SunburstCanvas` draws rings with a hole at 30% of the radius and then draws the current folder name at the center. `GraphicsContext.draw(_:at:anchor:)` does not clip to the hole. A name such as `disklens-fixture.rXNBaq` ran across the inner ring.

The hole radius is known before the label is drawn. The character budget is `max(10, Int(hole / 7))`, about one character per 7 points, with a floor so a tiny chart still shows a prefix. Longer names keep that many characters minus one and an ellipsis. The shortened string is what is drawn. The full name remains in the breadcrumb above the chart.

## Treemap labels

`TreemapCanvas` draws a name when a depth-0 rectangle is wider than 72 and taller than 28. The label was `.foregroundStyle(.white)`. Every `Palette` color is a mid-tone: documents are about `(0.86, 0.62, 0.24)`, and the gray "Other" fill is about `(0.42, 0.48, 0.56)`. White on those fails contrast. None of the fills are dark enough that white is the better choice, including the red video color.

The label is now `Color.black.opacity(0.82)`. That stays dark in both appearances. `foregroundStyle(.primary)` would have turned white again under the dark scheme, which is why the color is absolute black rather than a semantic label color.

## Progress paths

While a scan or a duplicate search is running, the current path was a two-line caption with no truncation mode. A long path wrapped at an arbitrary character. Both `MapView` and `DuplicatesView` now set `.truncationMode(.middle)` and center the line, still limited to two lines and 520 points wide.

## Duplicate toolbar

The results header put the folder path, the group count, Smart Selection, and "Move Selected to Trash…" in one `HStack`. The path had `.lineLimit(1)` but the same compression resistance as the buttons, so the button titles ellipsized first ("Move Selected to Trash…").

The path now has `.layoutPriority(-1)` and middle truncation. The group count, the unreadable count, and both buttons use `.fixedSize()`, so they keep their text and the path is what shrinks. The buttons still call `applySmartSelection` and `reviewPaths(selectedDuplicatePaths())`.

## What was checked

On the temp fixture, in the system's dark appearance:

- the sunburst center read `disklens-...` inside the hole, with the full name in the breadcrumb and the footer (`1.8 MB`, 8 files)
- the treemap labeled video, photos, backup, Library, archives, and documents, with dark text on the gold document tile
- the duplicate list showed the protected Library copy unselected and the review sheet showed that copy under "Protected locations"
