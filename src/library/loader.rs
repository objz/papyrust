use std::{fs, path::PathBuf};

use super::project::{Project, ProjectMeta};

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

pub fn discover_projects() -> Vec<Project> {
    let mut projects = Vec::new();

    for base_dir in resolve_paths() {
        if let Ok(entries) = fs::read_dir(base_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let project_json = path.join("project.json");

                    if project_json.exists() {
                        match parse(&project_json, &path) {
                            // Pass the directory path
                            Ok(project) => projects.push(project),
                            Err(e) => eprintln!("Failed to parse project at {:?}: {}", path, e),
                        }
                    }
                }
            }
        }
    }

    projects
}

fn parse(json_path: &PathBuf, project_dir: &PathBuf) -> Result<Project, String> {
    let content = fs::read_to_string(json_path)
        .map_err(|e| format!("Failed to read file {}: {}", json_path.display(), e))?;

    let project_metadata: ProjectMeta = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse JSON from {}: {}", json_path.display(), e))?;

    Ok(Project {
        meta: project_metadata,
        path: project_dir.to_string_lossy().to_string(),
    })
}
