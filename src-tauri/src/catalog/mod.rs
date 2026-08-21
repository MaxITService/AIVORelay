use once_cell::sync::Lazy;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct CatalogRoot {
    #[serde(default)]
    mirrors: Vec<String>,
    models: Vec<CatalogModel>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CatalogModel {
    pub id: String,
    #[serde(default)]
    pub revision: Option<String>,
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
    #[serde(default)]
    pub sha256: Option<String>,
}

impl CatalogModel {
    pub fn default_file(&self) -> Option<&CatalogFile> {
        self.files
            .iter()
            .find(|file| Some(file.quant.as_str()) == self.default_quant.as_deref())
            .or_else(|| self.files.first())
    }
}

static ROOT: Lazy<CatalogRoot> = Lazy::new(|| {
    let root: CatalogRoot = serde_json::from_str(include_str!("catalog.json"))
        .expect("bundled catalog.json should match the upstream schema");
    root
});

pub static CATALOG: Lazy<Vec<CatalogModel>> = Lazy::new(|| ROOT.models.clone());

pub struct MirrorFile {
    pub url: String,
    pub sha256: String,
    pub size_bytes: u64,
}

/// Return trusted mirror copies for a catalog file. A mirror is usable only
/// when the catalog pins both its immutable HF revision and its content hash.
pub fn mirror_fallbacks(model_id: &str) -> Vec<MirrorFile> {
    let Some((model, file)) = ROOT.models.iter().find_map(|model| {
        model
            .files
            .iter()
            .find(|file| format!("{}/{}", model.id, file.filename) == model_id)
            .map(|file| (model, file))
    }) else {
        return Vec::new();
    };

    let Some(revision) = model.revision.as_deref() else {
        return Vec::new();
    };
    let Some(sha256) = file.sha256.as_deref() else {
        return Vec::new();
    };

    ROOT.mirrors
        .iter()
        .map(|base| MirrorFile {
            url: format!(
                "{}/{}/{}/{}",
                base.trim_end_matches('/'),
                model.id,
                revision,
                file.filename
            ),
            sha256: sha256.to_string(),
            size_bytes: file.size_bytes,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::CATALOG;

    #[test]
    fn pure_diarization_models_are_not_downloadable() {
        assert!(
            CATALOG
                .iter()
                .all(|model| model.architecture.as_deref() != Some("sortformer")),
            "Sortformer produces speaker segments, not transcription text"
        );
    }
}
