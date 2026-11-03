//! `cargo xtask parity`: recompute the feature-parity summary in docs/parity-checklist.md from its row
//! table (each PowerPoint feature scored D / P / M, weighted by priority).

use std::path::Path;

pub use craft_xtask::parity::{rows, summary};

pub fn run(root: &Path) -> Result<(), String> {
    let config = craft_xtask::parity::Config { cargo: craft_xtask::Cargo { root: root.into(), ..Default::default() }, ..Default::default() };
    let path = root.join(&config.checklist_path);
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let (Some(_), Some(rows_start)) = (text.find(&config.summary_marker), text.find(&config.rows_marker)) else {
        return Err("docs/parity-checklist.md needs a `<!-- SUMMARY -->` marker before `## Rows`".into());
    };
    let updated = craft_xtask::parity::rewrite(&config, &text)?;
    std::fs::write(&path, updated).map_err(|e| e.to_string())?;
    print!("{}", summary(&rows(&text[rows_start..])));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scores_rows() {
        let t = "| Area | Feature | Priority | Status |\n|---|---|---|---|\n| A | x | P0 | D |\n| A | y | P2 | M |\n| B | z | P1 | P |\n";
        let r = rows(t);
        assert_eq!(r.len(), 3);
        let s = summary(&r);
        // (3·1 + 1·0 + 2·0.5) / 6 = 67%
        assert!(s.contains("**Weighted breadth parity: 67%**"), "{s}");
        assert!(s.contains("Open P0 items: ."));
    }
}
