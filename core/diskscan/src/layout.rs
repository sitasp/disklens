//! Geometry for the sunburst and the treemap.
//!
//! Both functions are pure: they do not touch the filesystem, so the SwiftUI
//! views can ask for a fresh layout on every resize without rescanning.
//! Angles are radians, with zero pointing east and the first slice starting
//! at 12 o'clock (`-π/2`), matching the usual disk-map convention.

use std::f64::consts::{PI, TAU};

use crate::category::FileCategory;
use crate::model::{NodeKind, ScanNode, SizeMetric};

/// One annular slice. Depth 0 is the centre disc (the node that was laid out).
#[derive(Debug, Clone, PartialEq)]
pub struct SunburstArc {
    pub path: String,
    pub name: String,
    pub depth: u32,
    pub start_angle: f64,
    pub end_angle: f64,
    /// Size in the requested metric.
    pub size: u64,
    /// This slice's share of its parent. The centre's fraction is 1.
    pub fraction_of_parent: f64,
    pub category: FileCategory,
    pub kind: NodeKind,
    /// `true` for the synthetic "Other" bucket that absorbs tiny slices.
    pub is_aggregate: bool,
}

/// One rectangle in a squarified treemap. Depth 0 rectangles are the immediate
/// children of the node that was laid out; deeper rectangles subdivide those.
#[derive(Debug, Clone, PartialEq)]
pub struct TreemapRect {
    pub path: String,
    pub name: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub size: u64,
    pub depth: u32,
    pub category: FileCategory,
    pub kind: NodeKind,
}

/// Lay out `node` as a sunburst.
///
/// `max_depth` is the deepest ring to emit. `0` returns only the centre.
/// Children that occupy less than `min_fraction` of their parent (and every
/// smaller sibling after the first one that qualifies) are folded into a
/// single "Other" slice so the rings stay clickable. Pass `0.0` to keep
/// every non-zero child.
#[must_use]
pub fn sunburst(
    node: &ScanNode,
    metric: SizeMetric,
    max_depth: u32,
    min_fraction: f64,
) -> Vec<SunburstArc> {
    let mut frame = SunburstFrame { metric, max_depth, min_fraction, arcs: Vec::new() };
    // Mathematical angles: 0 is east, increasing counter-clockwise.
    // `-π/2` is 12 o'clock, which is where the first slice starts.
    push_sunburst(node, 0, -PI / 2.0, -PI / 2.0 + TAU, 1.0, &mut frame);
    frame.arcs
}

struct SunburstFrame {
    metric: SizeMetric,
    max_depth: u32,
    min_fraction: f64,
    arcs: Vec<SunburstArc>,
}

fn push_sunburst(
    node: &ScanNode,
    depth: u32,
    start: f64,
    end: f64,
    fraction: f64,
    frame: &mut SunburstFrame,
) {
    let size = node.size(frame.metric);
    frame.arcs.push(arc_from_node(node, depth, start, end, fraction, size));
    if depth >= frame.max_depth || size == 0 {
        return;
    }
    let span = end - start;
    let mut kept: Vec<&ScanNode> = Vec::new();
    let mut other = 0u64;
    for child in &node.children {
        let child_size = child.size(frame.metric);
        if child_size == 0 {
            continue;
        }
        let child_fraction = child_size as f64 / size as f64;
        if child_fraction < frame.min_fraction {
            other = other.saturating_add(child_size);
        } else {
            kept.push(child);
        }
    }
    let mut angle = start;
    let last = kept.len().saturating_sub(1);
    for (index, child) in kept.iter().enumerate() {
        let child_size = child.size(frame.metric);
        let child_fraction = child_size as f64 / size as f64;
        let is_last = other == 0 && index == last;
        let next = if is_last { end } else { angle + span * child_fraction };
        push_sunburst(child, depth + 1, angle, next, child_fraction, frame);
        angle = next;
    }
    if other > 0 {
        frame.arcs.push(SunburstArc {
            path: String::new(),
            name: "Other".to_owned(),
            depth: depth + 1,
            start_angle: angle,
            end_angle: end,
            size: other,
            fraction_of_parent: other as f64 / size as f64,
            category: FileCategory::Other,
            kind: NodeKind::Directory,
            is_aggregate: true,
        });
    }
}

fn arc_from_node(
    node: &ScanNode,
    depth: u32,
    start: f64,
    end: f64,
    fraction: f64,
    size: u64,
) -> SunburstArc {
    SunburstArc {
        path: node.path.display().to_string(),
        name: node.name.clone(),
        depth,
        start_angle: start,
        end_angle: end,
        size,
        fraction_of_parent: fraction,
        category: node.category,
        kind: node.kind,
        is_aggregate: false,
    }
}

/// Squarified treemap of `node`'s descendants, fitting `(width, height)`.
///
/// `max_depth` counts levels of children (`1` shows only immediate children).
/// `padding` insets each cell before it is subdivided, in the same units as
/// `width`. Padding does not change the outer cell, so hit testing still uses
/// the full rectangle.
#[must_use]
pub fn treemap(
    node: &ScanNode,
    metric: SizeMetric,
    width: f64,
    height: f64,
    max_depth: u32,
    padding: f64,
) -> Vec<TreemapRect> {
    if width <= 0.0 || height <= 0.0 || max_depth == 0 {
        return Vec::new();
    }
    let mut frame = TreemapFrame { metric, max_depth, padding, rects: Vec::new() };
    lay_children(node, 0.0, 0.0, width, height, 0, &mut frame);
    frame.rects
}

struct TreemapFrame {
    metric: SizeMetric,
    max_depth: u32,
    padding: f64,
    rects: Vec<TreemapRect>,
}

fn lay_children(
    node: &ScanNode,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    depth: u32,
    frame: &mut TreemapFrame,
) {
    let total = node.size(frame.metric);
    if total == 0 || width <= 0.0 || height <= 0.0 {
        return;
    }
    let mut indexed: Vec<(usize, f64)> = node
        .children
        .iter()
        .enumerate()
        .filter_map(|(index, child)| {
            let size = child.size(frame.metric);
            if size == 0 {
                None
            } else {
                Some((index, size as f64))
            }
        })
        .collect();
    indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let sizes: Vec<f64> = indexed.iter().map(|(_, size)| *size).collect();
    let area = width * height;
    let scaled = normalize(&sizes, area);
    let placed = squarify(&scaled, x, y, width, height);
    for ((index, _), rect) in indexed.iter().zip(placed.iter()) {
        let child = &node.children[*index];
        frame.rects.push(TreemapRect {
            path: child.path.display().to_string(),
            name: child.name.clone(),
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
            size: child.size(frame.metric),
            depth,
            category: child.category,
            kind: child.kind,
        });
        if depth + 1 < frame.max_depth && child.kind == NodeKind::Directory {
            let inset = frame.padding.max(0.0);
            let inner_w = (rect.width - inset * 2.0).max(0.0);
            let inner_h = (rect.height - inset * 2.0).max(0.0);
            if inner_w > 0.0 && inner_h > 0.0 {
                lay_children(
                    child,
                    rect.x + inset,
                    rect.y + inset,
                    inner_w,
                    inner_h,
                    depth + 1,
                    frame,
                );
            }
        }
    }
}

#[derive(Clone, Copy)]
struct BoxRect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

fn normalize(sizes: &[f64], area: f64) -> Vec<f64> {
    let total: f64 = sizes.iter().sum();
    if total <= 0.0 {
        return vec![0.0; sizes.len()];
    }
    sizes.iter().map(|size| size / total * area).collect()
}

/// Squarify `areas` (which must already sum to `width * height`) into `rect`.
/// Ported from the reference algorithm used by the `squarify` Python package.
fn squarify(areas: &[f64], x: f64, y: f64, width: f64, height: f64) -> Vec<BoxRect> {
    if areas.is_empty() || width <= 0.0 || height <= 0.0 {
        return Vec::new();
    }
    if areas.len() == 1 {
        return vec![BoxRect { x, y, width, height }];
    }
    let mut split = 1;
    while split < areas.len()
        && worst_ratio(&areas[..split], width, height)
            >= worst_ratio(&areas[..=split], width, height)
    {
        split += 1;
    }
    let (row, rest) = areas.split_at(split);
    let mut placed = layout_row(row, x, y, width, height);
    let (next_x, next_y, next_w, next_h) = leftover(row, x, y, width, height);
    placed.extend(squarify(rest, next_x, next_y, next_w, next_h));
    placed
}

fn worst_ratio(areas: &[f64], width: f64, height: f64) -> f64 {
    layout_row(areas, 0.0, 0.0, width, height)
        .into_iter()
        .map(|rect| {
            if rect.width <= 0.0 || rect.height <= 0.0 {
                f64::MAX
            } else {
                let ratio = rect.width / rect.height;
                ratio.max(1.0 / ratio)
            }
        })
        .fold(1.0_f64, f64::max)
}

fn layout_row(areas: &[f64], x: f64, y: f64, width: f64, height: f64) -> Vec<BoxRect> {
    if width >= height {
        layout_vertical(areas, x, y, height)
    } else {
        layout_horizontal(areas, x, y, width)
    }
}

fn layout_vertical(areas: &[f64], x: f64, y: f64, height: f64) -> Vec<BoxRect> {
    let sum: f64 = areas.iter().sum();
    let thickness = if height > 0.0 { sum / height } else { 0.0 };
    let mut cursor = y;
    areas
        .iter()
        .map(|area| {
            let item_height = if thickness > 0.0 { area / thickness } else { 0.0 };
            let rect = BoxRect { x, y: cursor, width: thickness, height: item_height };
            cursor += item_height;
            rect
        })
        .collect()
}

fn layout_horizontal(areas: &[f64], x: f64, y: f64, width: f64) -> Vec<BoxRect> {
    let sum: f64 = areas.iter().sum();
    let thickness = if width > 0.0 { sum / width } else { 0.0 };
    let mut cursor = x;
    areas
        .iter()
        .map(|area| {
            let item_width = if thickness > 0.0 { area / thickness } else { 0.0 };
            let rect = BoxRect { x: cursor, y, width: item_width, height: thickness };
            cursor += item_width;
            rect
        })
        .collect()
}

fn leftover(areas: &[f64], x: f64, y: f64, width: f64, height: f64) -> (f64, f64, f64, f64) {
    let sum: f64 = areas.iter().sum();
    if width >= height {
        let thickness = if height > 0.0 { sum / height } else { 0.0 };
        (x + thickness, y, (width - thickness).max(0.0), height)
    } else {
        let thickness = if width > 0.0 { sum / width } else { 0.0 };
        (x, y + thickness, width, (height - thickness).max(0.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::category::FileCategory;
    use std::path::PathBuf;

    fn file(name: &str, size: u64) -> ScanNode {
        ScanNode {
            name: name.to_owned(),
            path: PathBuf::from(format!("/scan/{name}")),
            kind: NodeKind::File,
            category: FileCategory::Other,
            own_apparent: size,
            own_allocated: size,
            logical_apparent: size,
            unique_apparent: size,
            unique_allocated: size,
            file_count: 1,
            modified_unix_ms: None,
            inode: None,
            device: None,
            clone_id: None,
            shared_elsewhere: false,
            other_volume: false,
            error: None,
            children: Vec::new(),
        }
    }

    fn dir(name: &str, children: Vec<ScanNode>) -> ScanNode {
        let unique_apparent = children.iter().map(|child| child.unique_apparent).sum();
        ScanNode {
            name: name.to_owned(),
            path: PathBuf::from("/scan"),
            kind: NodeKind::Directory,
            category: FileCategory::Other,
            own_apparent: 0,
            own_allocated: 0,
            logical_apparent: unique_apparent,
            unique_apparent,
            unique_allocated: unique_apparent,
            file_count: children.iter().map(|child| child.file_count).sum(),
            modified_unix_ms: None,
            inode: None,
            device: None,
            clone_id: None,
            shared_elsewhere: false,
            other_volume: false,
            error: None,
            children,
        }
    }

    #[test]
    fn sunburst_children_fill_the_circle() {
        let root = dir("scan", vec![file("a", 75), file("b", 25)]);
        let arcs = sunburst(&root, SizeMetric::UniqueApparent, 1, 0.0);
        let ring: Vec<_> = arcs.iter().filter(|arc| arc.depth == 1).collect();
        assert_eq!(ring.len(), 2);
        let span: f64 = ring.iter().map(|arc| arc.end_angle - arc.start_angle).sum();
        assert!((span - TAU).abs() < 1e-9, "span {span} should be a full turn");
        assert!((ring[0].start_angle - (-PI / 2.0)).abs() < 1e-9);
        let big = ring.iter().find(|arc| arc.name == "a").unwrap();
        assert!(
            (big.fraction_of_parent - 0.75).abs() < 1e-9
                || (big.end_angle - big.start_angle - 0.75 * TAU).abs() < 1e-6
        );
    }

    #[test]
    fn sunburst_nested_slice_stays_inside_its_parent() {
        let child = dir("nested", vec![file("inner", 10)]);
        let mut child = child;
        child.unique_apparent = 10;
        child.path = PathBuf::from("/scan/nested");
        let mut root = dir("scan", vec![child, file("other", 30)]);
        root.unique_apparent = 40;
        let arcs = sunburst(&root, SizeMetric::UniqueApparent, 2, 0.0);
        let parent = arcs.iter().find(|arc| arc.name == "nested" && arc.depth == 1).unwrap();
        let inner = arcs.iter().find(|arc| arc.name == "inner").unwrap();
        assert_eq!(inner.depth, 2);
        assert!(inner.start_angle >= parent.start_angle - 1e-9);
        assert!(inner.end_angle <= parent.end_angle + 1e-9);
    }

    #[test]
    fn sunburst_folds_tiny_slices_and_skips_zeros() {
        let root = dir("scan", vec![file("big", 1000), file("tiny", 1), file("empty", 0)]);
        let arcs = sunburst(&root, SizeMetric::UniqueApparent, 1, 0.05);
        let names: Vec<_> =
            arcs.iter().filter(|arc| arc.depth == 1).map(|arc| arc.name.as_str()).collect();
        assert!(names.contains(&"big"));
        assert!(names.contains(&"Other"));
        assert!(!names.contains(&"tiny"));
        assert!(!names.contains(&"empty"));
        assert!(arcs.iter().any(|arc| arc.is_aggregate));
    }

    #[test]
    fn treemap_areas_match_sizes_and_stay_inside() {
        let root = dir("scan", vec![file("a", 1), file("b", 1), file("c", 1), file("d", 1)]);
        let rects = treemap(&root, SizeMetric::UniqueApparent, 100.0, 100.0, 1, 0.0);
        assert_eq!(rects.len(), 4);
        let area: f64 = rects.iter().map(|rect| rect.width * rect.height).sum();
        assert!((area - 10_000.0).abs() < 1e-6, "area {area}");
        for rect in &rects {
            assert!(rect.x >= -1e-6 && rect.y >= -1e-6);
            assert!(rect.x + rect.width <= 100.0 + 1e-6);
            assert!(rect.y + rect.height <= 100.0 + 1e-6);
            assert!((rect.width * rect.height - 2_500.0).abs() < 1e-4);
        }
        // Equal cells of a squarified map should not overlap.
        for (i, left) in rects.iter().enumerate() {
            for right in rects.iter().skip(i + 1) {
                let overlap_w =
                    (left.x + left.width).min(right.x + right.width) - left.x.max(right.x);
                let overlap_h =
                    (left.y + left.height).min(right.y + right.height) - left.y.max(right.y);
                let overlap = overlap_w.max(0.0) * overlap_h.max(0.0);
                assert!(
                    overlap < 1e-4,
                    "overlap {overlap} between {} and {}",
                    left.name,
                    right.name
                );
            }
        }
    }

    #[test]
    fn treemap_respects_proportion() {
        let root = dir("scan", vec![file("big", 3), file("small", 1)]);
        let rects = treemap(&root, SizeMetric::UniqueApparent, 200.0, 50.0, 1, 0.0);
        let big = rects.iter().find(|rect| rect.name == "big").unwrap();
        let small = rects.iter().find(|rect| rect.name == "small").unwrap();
        let big_area = big.width * big.height;
        let small_area = small.width * small.height;
        assert!((big_area / small_area - 3.0).abs() < 1e-6);
    }

    #[test]
    fn empty_inputs_produce_no_geometry() {
        let root = dir("scan", vec![]);
        assert!(sunburst(&root, SizeMetric::UniqueApparent, 2, 0.0)
            .iter()
            .all(|arc| arc.depth == 0));
        assert!(treemap(&root, SizeMetric::UniqueApparent, 10.0, 10.0, 1, 0.0).is_empty());
        assert!(treemap(
            &dir("scan", vec![file("a", 1)]),
            SizeMetric::UniqueApparent,
            0.0,
            10.0,
            1,
            0.0
        )
        .is_empty());
    }
}
