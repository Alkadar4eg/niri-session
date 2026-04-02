//! On-disk session format (`schema` version 1).

use std::collections::HashMap;

use niri_ipc::Output;
use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionFile {
    pub schema: u32,
    pub niri_version: String,
    pub outputs: HashMap<String, Output>,
    pub workspaces: Vec<WorkspaceEntry>,
    pub windows: Vec<WindowEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceEntry {
    pub id: u64,
    pub idx: u8,
    pub name: Option<String>,
    pub output: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowEntry {
    /// argv to restore (from `/proc/<pid>/cmdline` at save time).
    pub command: Vec<String>,
    pub app_id: Option<String>,
    pub title: Option<String>,
    pub output: String,
    pub workspace_idx: u8,
    /// 1-based column index in scrolling layout.
    pub column: usize,
    /// 1-based tile index within the column.
    pub tile: usize,
    pub is_floating: bool,
}

impl WindowEntry {
    pub fn sort_key(&self) -> (&str, u8, usize, usize) {
        (
            self.output.as_str(),
            self.workspace_idx,
            self.column,
            self.tile,
        )
    }
}

impl SessionFile {
    pub fn sorted_windows(&self) -> Vec<&WindowEntry> {
        let mut v: Vec<_> = self.windows.iter().collect();
        v.sort_by(|a, b| a.sort_key().cmp(&b.sort_key()));
        v
    }

    /// Группы подряд идущих тайловых окон с одним `(output, workspace_idx, column)` — для `--load`
    /// в колонку-стек. Плавающие окна — по одному в группе.
    pub fn column_groups<'a>(sorted: &[&'a WindowEntry]) -> Vec<Vec<&'a WindowEntry>> {
        let mut groups: Vec<Vec<&'a WindowEntry>> = Vec::new();
        for &w in sorted {
            if w.is_floating {
                groups.push(vec![w]);
                continue;
            }
            if let Some(last) = groups.last_mut() {
                let l0 = last[0];
                if !l0.is_floating
                    && l0.output == w.output
                    && l0.workspace_idx == w.workspace_idx
                    && l0.column == w.column
                {
                    last.push(w);
                    continue;
                }
            }
            groups.push(vec![w]);
        }
        groups
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_session() -> SessionFile {
        SessionFile {
            schema: SCHEMA_VERSION,
            niri_version: "test".into(),
            outputs: HashMap::new(),
            workspaces: vec![],
            windows: vec![
                WindowEntry {
                    command: vec!["b".into()],
                    app_id: None,
                    title: None,
                    output: "OUT".into(),
                    workspace_idx: 1,
                    column: 2,
                    tile: 1,
                    is_floating: false,
                },
                WindowEntry {
                    command: vec!["a".into()],
                    app_id: None,
                    title: None,
                    output: "OUT".into(),
                    workspace_idx: 1,
                    column: 1,
                    tile: 1,
                    is_floating: false,
                },
            ],
        }
    }

    #[test]
    fn sorted_windows_orders_by_output_workspace_column_tile() {
        let s = sample_session();
        let sorted = s.sorted_windows();
        assert_eq!(sorted.len(), 2);
        assert_eq!(sorted[0].command[0], "a");
        assert_eq!(sorted[1].command[0], "b");
    }

    #[test]
    fn column_groups_merge_same_column() {
        let w1 = WindowEntry {
            command: vec!["a".into()],
            app_id: None,
            title: None,
            output: "O".into(),
            workspace_idx: 1,
            column: 1,
            tile: 1,
            is_floating: false,
        };
        let w2 = WindowEntry {
            command: vec!["b".into()],
            app_id: None,
            title: None,
            output: "O".into(),
            workspace_idx: 1,
            column: 1,
            tile: 2,
            is_floating: false,
        };
        let w3 = WindowEntry {
            command: vec!["c".into()],
            app_id: None,
            title: None,
            output: "O".into(),
            workspace_idx: 1,
            column: 2,
            tile: 1,
            is_floating: false,
        };
        let s = SessionFile {
            windows: vec![w1, w2, w3],
            ..sample_session()
        };
        let sorted = s.sorted_windows();
        let g = SessionFile::column_groups(&sorted);
        assert_eq!(g.len(), 2);
        assert_eq!(g[0].len(), 2);
        assert_eq!(g[0][0].tile, 1);
        assert_eq!(g[0][1].tile, 2);
        assert_eq!(g[1].len(), 1);
    }

    #[test]
    fn session_json_roundtrip_empty_outputs() {
        let s = sample_session();
        let json = serde_json::to_string(&s).expect("serialize");
        let back: SessionFile = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.schema, s.schema);
        assert_eq!(back.windows.len(), s.windows.len());
        assert_eq!(back.windows[0].column, s.windows[0].column);
    }
}
