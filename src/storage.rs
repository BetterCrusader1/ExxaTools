//! Free space on the drives Steam actually writes to.
//!
//! A "disk write error" in the download is almost always one of these folders
//! running out of room, so the answer is a number, not a suggestion.

use std::path::PathBuf;

use serde_json::{json, Value};

pub fn free_bytes(path: &std::path::Path) -> Option<u64> {
    let output = std::process::Command::new("df").args(["-B1", "--output=avail"]).arg(path).output().ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    text.lines().nth(1)?.trim().parse().ok()
}

pub fn human(bytes: u64) -> String {
    let units = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < units.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit >= 3 {
        format!("{value:.1} {}", units[unit])
    } else {
        format!("{value:.0} {}", units[unit])
    }
}

/// The Steam root plus every extra library folder declared in libraryfolders.vdf.
pub fn library_dirs() -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    let Some(root) = crate::steamctl::root() else { return out };
    out.push(root.clone());
    for name in ["steamapps/libraryfolders.vdf", "config/libraryfolders.vdf"] {
        let path = root.join(name);
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        let Ok(tree) = crate::vdf::parse(&text) else { continue };
        let mut paths = Vec::new();
        crate::vdf::collect_paths(&tree, &mut paths);
        for entry in paths {
            let dir = PathBuf::from(entry);
            if dir.is_dir() && !out.iter().any(|seen| seen == &dir) {
                out.push(dir);
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

pub fn report() -> Value {
    let mut drives: Vec<Value> = Vec::new();
    let mut lowest: Option<(PathBuf, u64)> = None;
    for dir in library_dirs() {
        let Some(free) = free_bytes(&dir) else { continue };
        if lowest.as_ref().map(|(_, value)| free < *value).unwrap_or(true) {
            lowest = Some((dir.clone(), free));
        }
        drives.push(json!({
            "path": dir.display().to_string(),
            "free": free,
            "free_human": human(free),
        }));
    }
    json!({
        "drives": drives,
        "lowest": lowest.as_ref().map(|(path, free)| json!({
            "path": path.display().to_string(),
            "free": free,
            "free_human": human(*free),
        })),
    })
}

#[cfg(test)]
mod tests {
    use super::human;

    #[test]
    fn human_sizes() {
        assert_eq!(human(512), "512 B");
        assert_eq!(human(1536), "2 KiB");
        assert_eq!(human(120 * 1024 * 1024 * 1024), "120.0 GiB");
    }
}
