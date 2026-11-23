//! Human-readable sizes.
//!
//! Units are decimal, matching Finder: 1 KB is 1,000 bytes, not 1,024.

use crate::model::{ScanNode, SizeMetric};

/// Format `bytes` the way Finder does.
///
/// ```
/// assert_eq!(diskscan::format_bytes(0), "0 bytes");
/// assert_eq!(diskscan::format_bytes(1), "1 byte");
/// assert_eq!(diskscan::format_bytes(1_500), "1.5 KB");
/// assert_eq!(diskscan::format_bytes(1_000_000), "1 MB");
/// ```
#[must_use]
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["bytes", "KB", "MB", "GB", "TB", "PB"];
    if bytes < 1000 {
        return if bytes == 1 { "1 byte".to_owned() } else { format!("{bytes} bytes") };
    }
    // f64 is exact for every integer up to 2^53 (about 9 PB). Disk sizes past
    // that are outside the range this UI needs to render precisely.
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    let text = if value >= 100.0 || (value - value.round()).abs() < 0.05 {
        format!("{value:.0}")
    } else {
        format!("{value:.1}")
    };
    format!("{text} {}", UNITS[unit])
}

/// Indent-outline of a tree, largest children first, up to `max_depth`.
#[must_use]
pub fn format_tree(node: &ScanNode, metric: SizeMetric, max_depth: usize) -> String {
    let mut out = String::new();
    write_node(&mut out, node, metric, 0, max_depth);
    out
}

fn write_node(
    out: &mut String,
    node: &ScanNode,
    metric: SizeMetric,
    depth: usize,
    max_depth: usize,
) {
    let indent = "  ".repeat(depth);
    let suffix = if node.shared_elsewhere {
        " (shared)"
    } else if node.kind == crate::model::NodeKind::Symlink {
        " -> symlink"
    } else if node.error.is_some() {
        " (unreadable)"
    } else {
        ""
    };
    out.push_str(&format!(
        "{indent}{size:>10}  {name}{suffix}\n",
        size = format_bytes(node.size(metric)),
        name = node.name,
    ));
    if depth >= max_depth {
        return;
    }
    for child in &node.children {
        write_node(out, child, metric, depth + 1, max_depth);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finder_style_units() {
        assert_eq!(format_bytes(0), "0 bytes");
        assert_eq!(format_bytes(1), "1 byte");
        assert_eq!(format_bytes(999), "999 bytes");
        assert_eq!(format_bytes(1_000), "1 KB");
        assert_eq!(format_bytes(1_500), "1.5 KB");
        assert_eq!(format_bytes(1_000_000), "1 MB");
        assert_eq!(format_bytes(2_500_000_000), "2.5 GB");
    }
}
