use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProjectType {
    Video,
    Image,
    Html,
    Unssupported,
}

#[derive(Deserialize)]
pub struct Project {
    title: Option<String>,
    description: Option<String>,
    tags: Option<Vec<String>>,
    file_type: Option<ProjectType>,
    preview: Option<String>,
    file: Option<String>,
}
