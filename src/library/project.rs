use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "lowercase")]
pub enum ProjectType {
    #[serde(alias = "Web")]
    Web,
    #[serde(alias = "Video")]
    Video,
    #[serde(alias = "Application")]
    Application,
    #[serde(alias = "Scene")]
    Scene,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ProjectMeta {
    pub title: Option<String>,
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
    #[serde(rename = "type")]
    pub file_type: Option<ProjectType>,
    pub preview: Option<String>,
    pub file: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Project {
    pub meta: ProjectMeta,
    pub path: String,
}
