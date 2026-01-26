use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct ListDirectoriesRequest {
    pub path: Option<String>,
}

#[derive(Serialize)]
pub struct DirectoryEntry {
    pub name: String,
    pub path: String,
}
