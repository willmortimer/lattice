//! Workspace folder templates applied at `init` time.
//!
//! Templates are one-time scaffolds. Once instantiated, every created file
//! belongs to the user; Lattice does not retain ownership of template content.

use std::path::Path;

use crate::{Error, Result};

/// Built-in scaffolding choices for a new workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceTemplate {
    /// Purpose-based daily workspace (default for `~/Lattice/Workspaces/Personal`).
    Personal,
    /// One finite, compound project with mixed resource types.
    Project,
    /// Sources, notes, data, experiments, and outputs.
    Research,
    /// Analytical work organized around sources, queries, notebooks, and reports.
    DataLab,
    /// Legacy lightweight team / product workspace.
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
            WorkspaceTemplate::Project => "project",
            WorkspaceTemplate::Research => "research",
            WorkspaceTemplate::DataLab => "data-lab",
            WorkspaceTemplate::Team => "team",
            WorkspaceTemplate::Demo => "demo",
            WorkspaceTemplate::Blank => "blank",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            WorkspaceTemplate::Personal => "Personal",
            WorkspaceTemplate::Project => "Project",
            WorkspaceTemplate::Research => "Research",
            WorkspaceTemplate::DataLab => "Data Lab",
            WorkspaceTemplate::Team => "Team",
            WorkspaceTemplate::Demo => "First Look",
            WorkspaceTemplate::Blank => "Blank",
        }
    }

    pub fn category(self) -> &'static str {
        match self {
            WorkspaceTemplate::Personal => "Everyday",
            WorkspaceTemplate::Project => "Focused work",
            WorkspaceTemplate::Research => "Knowledge",
            WorkspaceTemplate::DataLab => "Analysis",
            WorkspaceTemplate::Team => "Collaboration",
            WorkspaceTemplate::Demo => "Sample",
            WorkspaceTemplate::Blank => "Advanced",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            WorkspaceTemplate::Personal => {
                "Capture ideas, run projects, manage ongoing areas, and keep a durable library."
            }
            WorkspaceTemplate::Project => {
                "Plan and deliver one outcome with decisions, research, working files, data, and outputs together."
            }
            WorkspaceTemplate::Research => {
                "Move from questions and sources through notes, experiments, analysis, and published outputs."
            }
            WorkspaceTemplate::DataLab => {
                "Organize data sources, queries, notebooks, dashboards, reports, and reusable analysis."
            }
            WorkspaceTemplate::Team => {
                "Projects, docs, meetings, research, and archive with a shared Home page."
            }
            WorkspaceTemplate::Demo => {
                "A curated sample with linked pages and a canvas for a quick first look."
            }
            WorkspaceTemplate::Blank => {
                "Start with only lattice.yaml and shape the workspace yourself."
            }
        }
    }

    pub fn recommended(self) -> bool {
        self == WorkspaceTemplate::Personal
    }

    pub fn preview_paths(self) -> &'static [&'static str] {
        match self {
            WorkspaceTemplate::Personal => &[
                "Home.md",
                "Welcome.md",
                "Inbox/",
                "Projects/",
                "Areas/",
                "Library/",
                "Journal/",
            ],
            WorkspaceTemplate::Project => &[
                "Home.md",
                "Brief.md",
                "Plan.md",
                "Decisions/",
                "Working/",
                "Data/",
                "Outputs/",
            ],
            WorkspaceTemplate::Research => &[
                "Home.md",
                "Questions.md",
                "Sources/",
                "Notes/",
                "Data/",
                "Experiments/",
                "Outputs/",
            ],
            WorkspaceTemplate::DataLab => &[
                "Home.md",
                "Sources/",
                "Data/",
                "Queries/",
                "Notebooks/",
                "Dashboards/",
                "Reports/",
            ],
            WorkspaceTemplate::Team => &["Home.md", "Projects/", "Docs/", "Meetings/", "Research/"],
            WorkspaceTemplate::Demo => &[
                "Home.md",
                "Product/Vision.md",
                "Research/Competitor Analysis.md",
                "Canvases/Product Strategy.canvas",
            ],
            WorkspaceTemplate::Blank => &["lattice.yaml"],
        }
    }

    pub fn parse(id: &str) -> Option<Self> {
        match id.trim().to_ascii_lowercase().as_str() {
            "personal" | "default" => Some(WorkspaceTemplate::Personal),
            "project" => Some(WorkspaceTemplate::Project),
            "research" => Some(WorkspaceTemplate::Research),
            "data-lab" | "data_lab" | "datalab" | "data" => Some(WorkspaceTemplate::DataLab),
            "team" | "work" => Some(WorkspaceTemplate::Team),
            "demo" | "sample" | "first-look" => Some(WorkspaceTemplate::Demo),
            "blank" | "empty" | "none" => Some(WorkspaceTemplate::Blank),
            _ => None,
        }
    }

    /// Templates shown in the normal new-workspace gallery.
    ///
    /// Team remains available to existing callers while it is still a folder
    /// preset, and Demo is a sample workspace rather than an organizational
    /// choice, so neither appears here.
    pub fn gallery() -> &'static [WorkspaceTemplate] {
        &[
            WorkspaceTemplate::Personal,
            WorkspaceTemplate::Project,
            WorkspaceTemplate::Research,
            WorkspaceTemplate::DataLab,
            WorkspaceTemplate::Blank,
        ]
    }

    fn folders(self) -> &'static [&'static str] {
        match self {
            WorkspaceTemplate::Personal => &[
                "Inbox",
                "Projects",
                "Areas",
                "Library",
                "Journal",
                "Templates",
                "Archive",
            ],
            WorkspaceTemplate::Project => &[
                "Decisions",
                "Research",
                "Working",
                "Data",
                "Outputs",
                "Archive",
            ],
            WorkspaceTemplate::Research => &[
                "Inbox",
                "Sources",
                "Notes",
                "Data",
                "Experiments",
                "Outputs",
                "Bibliography",
                "Archive",
            ],
            WorkspaceTemplate::DataLab => &[
                "Sources",
                "Data",
                "Queries",
                "Notebooks",
                "Dashboards",
                "Reports",
                "Archive",
            ],
            WorkspaceTemplate::Team => &["Projects", "Docs", "Meetings", "Research", "Archive"],
            WorkspaceTemplate::Demo => &[
                "Inbox", "Projects", "Product", "Research", "Canvases", "Archive",
            ],
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

## Capture

- [[Inbox]]
- Create a quick note with **⌘N** or **Ctrl+N**

## Active work

- [[Projects]]

## Ongoing areas

- [[Areas]]

## Reference

- [[Library]]

## Start here

- [[Welcome]]

## Recent

Recent resources appear here when opened in Lattice.
"#,
            ),
            WorkspaceTemplate::Project => Some(
                r#"---
title: Home
---

# Project home

## Goal

Describe the outcome this project should produce.

## Current status

- Define the next milestone in [[Plan]].
- Capture constraints and context in [[Brief]].

## Key resources

- [[Brief]]
- [[Plan]]
- `Decisions/`
- `Research/`
- `Working/`
- `Data/`
- `Outputs/`

## Next actions

- [ ] Clarify the goal
- [ ] Choose the first deliverable
- [ ] Record the next decision
"#,
            ),
            WorkspaceTemplate::Research => Some(
                r#"---
title: Home
---

# Research home

## Research question

Start with [[Questions]].

## Workflow

1. Capture unprocessed material in [[Inbox]].
2. Keep original references in `Sources/`.
3. Develop your own thinking in `Notes/`.
4. Put datasets and measurements in `Data/`.
5. Keep reproducible work in `Experiments/`.
6. Publish durable results from `Outputs/`.
"#,
            ),
            WorkspaceTemplate::DataLab => Some(
                r#"---
title: Home
---

# Data Lab

## Start

- Register source material in `Sources/`.
- Keep canonical datasets in `Data/`.
- Save reusable SQL and transformations in `Queries/`.
- Put exploratory and reproducible computation in `Notebooks/`.

## Share

- Build interactive summaries in `Dashboards/`.
- Publish narrative findings from `Reports/`.

Keep raw inputs, computation, and conclusions connected without forcing them
into one file format.
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

# First Look

A sample Lattice workspace. Try search (**⌘K**), the canvas under `Canvases/`,
and quick notes (**⌘N**).

## Map

| Path | Kind |
| --- | --- |
| [[Product/Vision]] | page |
| [[Product/Roadmap]] | page |
| [[Research/Competitor Analysis]] | page |
| `Canvases/Product Strategy.canvas` | canvas |
"#,
            ),
            WorkspaceTemplate::Blank => None,
        }
    }

    fn seed_files(self) -> &'static [(&'static str, &'static str)] {
        match self {
            WorkspaceTemplate::Personal => &[(
                "Welcome.md",
                r#"---
title: Welcome
---

# Welcome to Lattice

Your workspace is an ordinary folder. You can edit these files with Lattice,
VS Code, a terminal, or any other compatible tool.

## Try these three things

1. Press **⌘N** or **Ctrl+N** to capture a note into [[Inbox]].
2. Create a table from the command palette.
3. Put a page, table, and canvas inside the same project folder.

## How this workspace is organized

- `Inbox/` collects unprocessed material.
- `Projects/` contains finite work with an intended outcome.
- `Areas/` contains ongoing responsibilities without a finish line.
- `Library/` contains durable reference material.
- `Journal/` contains optional dated and periodic notes.
- `Templates/` contains page and project starters you own.
- `Archive/` contains inactive work that should remain searchable.

Delete this page whenever you no longer need it.
"#,
            )],
            WorkspaceTemplate::Project => &[
                (
                    "Brief.md",
                    r#"---
title: Brief
---

# Brief

## Outcome

What will exist when this project is complete?

## Context

Why does this matter now?

## Constraints

- Time
- Scope
- Dependencies
"#,
                ),
                (
                    "Plan.md",
                    r#"---
title: Plan
---

# Plan

## Milestones

- [ ] Define
- [ ] Build
- [ ] Review
- [ ] Deliver

## Next actions

- [ ] Add the first concrete action
"#,
                ),
            ],
            WorkspaceTemplate::Research => &[(
                "Questions.md",
                r#"---
title: Questions
---

# Questions

## Primary question

What are you trying to learn, explain, or decide?

## Supporting questions

- What evidence would change your view?
- Which sources or datasets are missing?
- What assumptions need testing?
"#,
            )],
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

/// Create template folders, landing page, and seed files.
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
    fn personal_template_uses_purpose_based_folders_and_separate_welcome() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("ws");
        init_with_template(&root, "Test", WorkspaceTemplate::Personal).unwrap();

        for folder in [
            "Inbox",
            "Projects",
            "Areas",
            "Library",
            "Journal",
            "Templates",
            "Archive",
        ] {
            assert!(root.join(folder).is_dir(), "missing {folder}");
        }
        assert!(!root.join("Product").exists());
        assert!(!root.join("Canvases").exists());
        assert!(root.join("Home.md").is_file());
        assert!(root.join("Welcome.md").is_file());

        let home = std::fs::read_to_string(root.join("Home.md")).unwrap();
        let welcome = std::fs::read_to_string(root.join("Welcome.md")).unwrap();
        assert!(home.contains("## Capture"));
        assert!(!home.contains("How this workspace is organized"));
        assert!(welcome.contains("How this workspace is organized"));

        let ws = Workspace::open(&root).unwrap();
        assert_eq!(ws.manifest().title, "Test");
    }

    #[test]
    fn project_research_and_data_lab_templates_create_distinct_workflows() {
        let dir = tempfile::tempdir().unwrap();

        let project = dir.path().join("project");
        init_with_template(&project, "Project", WorkspaceTemplate::Project).unwrap();
        assert!(project.join("Brief.md").is_file());
        assert!(project.join("Decisions").is_dir());
        assert!(project.join("Outputs").is_dir());

        let research = dir.path().join("research");
        init_with_template(&research, "Research", WorkspaceTemplate::Research).unwrap();
        assert!(research.join("Questions.md").is_file());
        assert!(research.join("Sources").is_dir());
        assert!(research.join("Experiments").is_dir());

        let data_lab = dir.path().join("data-lab");
        init_with_template(&data_lab, "Data Lab", WorkspaceTemplate::DataLab).unwrap();
        assert!(data_lab.join("Queries").is_dir());
        assert!(data_lab.join("Notebooks").is_dir());
        assert!(data_lab.join("Dashboards").is_dir());
    }

    #[test]
    fn gallery_excludes_legacy_team_and_sample_workspace() {
        assert!(WorkspaceTemplate::gallery().contains(&WorkspaceTemplate::Personal));
        assert!(WorkspaceTemplate::gallery().contains(&WorkspaceTemplate::Project));
        assert!(!WorkspaceTemplate::gallery().contains(&WorkspaceTemplate::Team));
        assert!(!WorkspaceTemplate::gallery().contains(&WorkspaceTemplate::Demo));
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
