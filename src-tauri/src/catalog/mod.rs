use once_cell::sync::Lazy;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct CatalogRoot {
    models: Vec<CatalogModel>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CatalogModel {
    pub id: String,
    pub name: String,
    pub description: String,
    #[allow(dead_code)]
    pub architecture: Option<String>,
    pub languages: Vec<String>,
    pub capabilities: CatalogCaps,
    pub speed_score: Option<f32>,
    pub accuracy_score: Option<f32>,
    pub files: Vec<CatalogFile>,
    pub default_quant: Option<String>,
    #[serde(default)]
    pub recommended: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CatalogCaps {
    pub streaming: bool,
    pub translate: bool,
    pub lang_detect: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CatalogFile {
    pub filename: String,
    pub quant: String,
    pub size_bytes: u64,
}

impl CatalogModel {
    pub fn default_file(&self) -> Option<&CatalogFile> {
        self.files
            .iter()
            .find(|file| Some(file.quant.as_str()) == self.default_quant.as_deref())
            .or_else(|| self.files.first())
    }
}

pub static CATALOG: Lazy<Vec<CatalogModel>> = Lazy::new(|| {
    let root: CatalogRoot = serde_json::from_str(include_str!("catalog.json"))
        .expect("bundled catalog.json should match the upstream schema");
    root.models
});
