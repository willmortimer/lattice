//! Workspace folder templates applied at `init` time.
//!
//! Templates create folder scaffolding plus a landing page. The `demo`
//! template also seeds a few sample pages and a canvas so the browser
//! shell and first-run exploration have something interesting to open.

use std::path::Path;

use crate::{Error, Result};

/// Built-in scaffolding choices for a new workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceTemplate {
    /// PARA-inspired personal workspace (default for `~/Lattice/Workspaces/Personal`).
    Personal,
    /// Lightweight team / product workspace.
    Team,
    /// Sample content for demos and the browser-only shell.
    Demo,
    /// Manifest only — no folders or landing page.
    Blank,
}

impl WorkspaceTemplate {
    pub fn id(self) -> &'static str {
        match self {
            WorkspaceTemplate::Personal => "personal",
            WorkspaceTemplate::Team => "team",
            WorkspaceTemplate::Demo => "demo",
            WorkspaceTemplate::Blank => "blank",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            WorkspaceTemplate::Personal => "Personal",
            WorkspaceTemplate::Team => "Team",
            WorkspaceTemplate::Demo => "Demo",
            WorkspaceTemplate::Blank => "Blank",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            WorkspaceTemplate::Personal => {
                "Inbox, Projects, Product, Research, Notebooks, Canvases, Resources, Archive."
            }
            WorkspaceTemplate::Team => {
                "Projects, Docs, Meetings, Research, and Archive — with a Home page."
            }
            WorkspaceTemplate::Demo => {
                "Personal layout plus sample pages and a canvas (browser / first-look)."
            }
            WorkspaceTemplate::Blank => "Empty workspace: just lattice.yaml.",
        }
    }

    pub fn parse(id: &str) -> Option<Self> {
        match id.trim().to_ascii_lowercase().as_str() {
            "personal" | "default" => Some(WorkspaceTemplate::Personal),
            "team" | "work" => Some(WorkspaceTemplate::Team),
            "demo" | "sample" => Some(WorkspaceTemplate::Demo),
            "blank" | "empty" | "none" => Some(WorkspaceTemplate::Blank),
            _ => None,
        }
    }

    pub fn all() -> &'static [WorkspaceTemplate] {
        &[
            WorkspaceTemplate::Personal,
            WorkspaceTemplate::Team,
            WorkspaceTemplate::Demo,
            WorkspaceTemplate::Blank,
        ]
    }

    fn folders(self) -> &'static [&'static str] {
        match self {
            WorkspaceTemplate::Personal | WorkspaceTemplate::Demo => &[
                "Inbox",
                "Projects",
                "Product",
                "Research",
                "Notebooks",
                "Canvases",
                "Resources",
                "Archive",
            ],
            WorkspaceTemplate::Team => &["Projects", "Docs", "Meetings", "Research", "Archive"],
            WorkspaceTemplate::Blank => &[],
        }
    }

    fn home_markdown(self) -> Option<&'static str> {
        match self {
            WorkspaceTemplate::Personal => Some(
                r#"---
title: Home
---

# Home

This is your Lattice workspace. Everything here is an ordinary folder of files.

## Where things go

| Folder | Use it for |
| --- | --- |
| `Inbox/` | Quick captures — **⌘N** / **Ctrl+N** |
| `Projects/` | Active work with a finish line |
| `Product/` | Vision, roadmap, specs |
| `Research/` | Discovery and competitive notes |
| `Notebooks/` | Analysis notebooks (`.ipynb`) |
| `Canvases/` | Spatial boards (`.canvas`) |
| `Resources/` | Reference you revisit |
| `Archive/` | Finished or cold work |

Open **⌘K** to search, **⌘P** for the command palette.
"#,
            ),
            WorkspaceTemplate::Team => Some(
                r#"---
title: Home
---

# Home

Shared workspace for product and team notes — all plain files on disk.

## Where things go

| Folder | Use it for |
| --- | --- |
| `Projects/` | Active initiatives |
| `Docs/` | Specs, ADRs, and lasting write-ups |
| `Meetings/` | Agendas and notes |
| `Research/` | Discovery and competitive notes |
| `Archive/` | Completed work |

Use **⌘K** to search and **⌘P** for the command palette.
"#,
            ),
            WorkspaceTemplate::Demo => Some(
                r#"---
title: Home
---

# Home

A sample Lattice workspace. Try search (**⌘K**), the canvas under `Canvases/`, and quick notes (**⌘N**).

## Map

| Path | Kind |
| --- | --- |
| [[Product/Vision]] | page |
| [[Product/Roadmap]] | page |
| [[Research/Competitor Analysis]] | page |
| `Canvases/Product Strategy.canvas` | canvas |

Folders like `Notebooks/` and `Archive/` are ready for files you add later.
"#,
            ),
            WorkspaceTemplate::Blank => None,
        }
    }

    /// Extra seed files for the demo template (path, contents).
    fn seed_files(self) -> &'static [(&'static str, &'static str)] {
        match self {
            WorkspaceTemplate::Demo => &[
                (
                    "Product/Vision.md",
                    r#"---
title: Vision
---

# Vision

A fast local workspace that treats documents, data, notebooks, and canvases as ordinary files.

See also [[Product/Roadmap]] and [[Research/Competitor Analysis]].
"#,
                ),
                (
                    "Product/Roadmap.md",
                    r#"---
title: Roadmap
---

# Roadmap

1. Daily-driver editing and search
2. First data-app surface
3. Capture from anywhere

Back to [[Home]].
"#,
                ),
                (
                    "Research/Competitor Analysis.md",
                    r#"---
title: Competitor Analysis
tags: [research]
---

# Competitor Analysis

| Tool | Keeps | Traps |
| --- | --- | --- |
| Obsidian | plain files | rich data |
| Notion | interaction | your files |
| Airtable | typed records | API lock-in |

#research
"#,
                ),
                (
                    "Canvases/Product Strategy.canvas",
                    r#"{
  "nodes": [
    {
      "id": "intro",
      "type": "text",
      "x": 60,
      "y": 60,
      "width": 260,
      "height": 120,
      "text": "Sample canvas — double-click a file node to open it."
    },
    {
      "id": "vision",
      "type": "file",
      "file": "Product/Vision.md",
      "x": 360,
      "y": 60,
      "width": 220,
      "height": 120
    },
    {
      "id": "roadmap",
      "type": "file",
      "file": "Product/Roadmap.md",
      "x": 620,
      "y": 60,
      "width": 220,
      "height": 120
    }
  ],
  "edges": [
    { "id": "e1", "fromNode": "intro", "toNode": "vision" },
    { "id": "e2", "fromNode": "vision", "toNode": "roadmap" }
  ]
}
"#,
                ),
            ],
            _ => &[],
        }
    }
}

/// Create template folders, landing page, and any demo seed files.
pub fn apply_template(workspace_root: &Path, template: WorkspaceTemplate) -> Result<()> {
    for folder in template.folders() {
        let path = workspace_root.join(folder);
        std::fs::create_dir_all(&path).map_err(|e| Error::io(&path, e))?;
    }
    if let Some(body) = template.home_markdown() {
        let path = workspace_root.join("Home.md");
        if !path.exists() {
            std::fs::write(&path, body).map_err(|e| Error::io(&path, e))?;
        }
    }
    for (rel, body) in template.seed_files() {
        let path = workspace_root.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;
        }
        if !path.exists() {
            std::fs::write(&path, body).map_err(|e| Error::io(&path, e))?;
        }
    }
    Ok(())
}

/// `Workspace::init` + [`apply_template`].
pub fn init_with_template(
    root: &Path,
    title: impl Into<String>,
    template: WorkspaceTemplate,
) -> Result<crate::Workspace> {
    let ws = crate::Workspace::init(root, title)?;
    apply_template(ws.root(), template)?;
    Ok(ws)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Workspace;

    #[test]
    fn personal_template_creates_folders_and_home() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("ws");
        init_with_template(&root, "Test", WorkspaceTemplate::Personal).unwrap();
        assert!(root.join("Product").is_dir());
        assert!(root.join("Canvases").is_dir());
        assert!(root.join("Home.md").is_file());
        let ws = Workspace::open(&root).unwrap();
        assert_eq!(ws.manifest().title, "Test");
    }

    #[test]
    fn demo_template_seeds_sample_pages_and_canvas() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("ws");
        init_with_template(&root, "Demo", WorkspaceTemplate::Demo).unwrap();
        assert!(root.join("Product/Vision.md").is_file());
        assert!(root.join("Canvases/Product Strategy.canvas").is_file());
    }

    #[test]
    fn blank_template_is_manifest_only() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("ws");
        init_with_template(&root, "Blank", WorkspaceTemplate::Blank).unwrap();
        assert!(root.join("lattice.yaml").is_file());
        assert!(!root.join("Home.md").exists());
        assert!(!root.join("Inbox").exists());
    }
}
