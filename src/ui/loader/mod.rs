use crate::ui::loader::project::{Project, ProjectMeta};
use std::{fs, path::PathBuf};

pub mod project;

const WALLPAPER_ENGINE_ID: &str = "431960";
const WORKSHOP_PATHS: [&str; 4] = [
    "~/.steam/steam/steamapps/workshop",
    "~/.local/share/Steam/steamapps/workshop",
    "~/.var/app/com.valvesoftware.Steam/.local/share/Steam/steamapps/workshop",
    "~/snap/steam/common/.local/share/Steam/steamapps/workshop",
];

fn resolve_paths() -> Vec<PathBuf> {
    WORKSHOP_PATHS
        .iter()
        .map(|p| shellexpand::tilde(p).to_string())
        .map(PathBuf::from)
        .filter(|p| p.exists())
        .map(|p| p.join("content").join(WALLPAPER_ENGINE_ID))
        .filter(|p| p.exists())
        .collect()
}

pub struct Loader {
    project_paths: Vec<PathBuf>,
    current: usize,
}

impl Loader {
    pub fn new() -> Self {
        let mut paths = Vec::new();

        for base in resolve_paths() {
            if let Ok(entries) = fs::read_dir(base) {
                for entry in entries.flatten() {
                    let dir = entry.path();
                    if dir.is_dir() && dir.join("project.json").exists() {
                        paths.push(dir);
                    }
                }
            }
        }

        Loader {
            project_paths: paths,
            current: 0,
        }
    }

    pub fn next(&mut self) -> Option<Result<Project, String>> {
        if self.current >= self.project_paths.len() {
            None
        } else {
            let dir = self.project_paths[self.current].clone();
            self.current += 1;
            let path = dir.join("project.json");
            Some(parse(&path, &dir))
        }
    }
}

fn parse(path: &PathBuf, dir: &PathBuf) -> Result<Project, String> {
    let content = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;

    let meta: ProjectMeta = serde_json::from_str(&content)
        .map_err(|e| format!("JSON parse error in {}: {}", path.display(), e))?;

    Ok(Project {
        meta,
        path: dir.to_string_lossy().to_string(),
    })
}
