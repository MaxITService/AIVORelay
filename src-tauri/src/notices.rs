//! Active failures are separate from the frontend's immutable session history.
//! Events and snapshots carry a revision; rows have unique IDs for safe dismissal.
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct Notice {
    pub id: u64,
    pub source: String,
    pub level: String,
    pub title: String,
    pub title_key: Option<String>,
    pub detail: Option<String>,
    pub target: Option<String>,
    pub generation: u64,
}
#[derive(Clone, Default, Serialize, Type)]
pub struct NoticeSnapshot {
    pub revision: u64,
    pub notices: Vec<Notice>,
    pub raised: Option<Notice>,
    pub open_requested: bool,
}
#[derive(Default)]
struct Inner {
    snapshot: NoticeSnapshot,
    next_id: u64,
    runs: HashMap<String, u64>,
}
#[derive(Default)]
pub struct NoticeStore(Mutex<Inner>);

impl Inner {
    fn begin(&mut self, source: &str, generation: u64) {
        self.runs.insert(source.into(), generation);
    }
    fn raise(&mut self, mut notice: Notice, generation: Option<u64>) -> bool {
        let current = self.runs.get(&notice.source).copied().unwrap_or(0);
        if generation.is_some_and(|value| value != current) { return false; }
        self.next_id += 1;
        notice.id = self.next_id;
        notice.generation = current;
        self.snapshot.notices.retain(|old| old.source != notice.source);
        self.snapshot.notices.insert(0, notice.clone());
        self.snapshot.notices.truncate(100);
        self.snapshot.revision += 1;
        self.snapshot.raised = Some(notice);
        true
    }
    fn remove(&mut self, predicate: impl Fn(&Notice) -> bool) -> bool {
        let before = self.snapshot.notices.len();
        self.snapshot.notices.retain(|notice| !predicate(notice));
        if self.snapshot.notices.len() == before { return false; }
        self.snapshot.revision += 1;
        self.snapshot.raised = None;
        true
    }
    fn succeed(&mut self, source: &str, generation: u64) -> bool {
        if self.runs.get(source) != Some(&generation) { return false; }
        // A warning raised by this run remains even if it delivered usable text.
        self.remove(|notice| notice.source == source && notice.generation < generation)
    }
}
fn update(app: &AppHandle, change: impl FnOnce(&mut Inner) -> bool) {
    let Some(store) = app.try_state::<NoticeStore>() else { return; };
    let changed = {
        let mut inner = store.0.lock().unwrap_or_else(|e| e.into_inner());
        let changed = change(&mut inner);
        if changed { let _ = app.emit("active-notices-changed", inner.snapshot.clone()); }
        changed
    };
    // Never enter tray code while holding the notice lock.
    if changed { crate::tray::refresh_tray_icon(app); }
}
pub fn begin(app: &AppHandle, source: &str, generation: u64) {
    update(app, |inner| { inner.begin(source, generation); false });
}
pub fn begin_next(app: &AppHandle, source: &str) -> u64 {
    let Some(store) = app.try_state::<NoticeStore>() else { return 0; };
    let mut inner = store.0.lock().unwrap_or_else(|e| e.into_inner());
    let generation = inner.runs.get(source).copied().unwrap_or(0) + 1;
    inner.begin(source, generation);
    generation
}
pub fn succeed(app: &AppHandle, source: &str, generation: u64) {
    update(app, |inner| inner.succeed(source, generation));
}
pub fn raise(app: &AppHandle, source: &str, generation: Option<u64>, level: &str,
    title: &str, title_key: Option<&str>, detail: Option<String>, target: Option<&str>) {
    update(app, |inner| inner.raise(Notice { id: 0, source: source.into(), level: level.into(),
        title: title.into(), title_key: title_key.map(str::to_string), detail,
        target: target.map(str::to_string), generation: 0 }, generation));
}
pub fn request_open(app: &AppHandle) {
    if let Some(store) = app.try_state::<NoticeStore>() {
        let mut inner = store.0.lock().unwrap_or_else(|e| e.into_inner());
        inner.snapshot.open_requested = true;
    }
    let _ = app.emit("open-active-notices", ());
}
pub fn headline(app: &AppHandle) -> Option<(String, bool)> {
    let store = app.try_state::<NoticeStore>()?;
    let inner = store.0.lock().unwrap_or_else(|e| e.into_inner());
    inner.snapshot.notices.iter().find(|n| n.level == "error")
        .or(inner.snapshot.notices.first()).map(|n| (n.title.clone(), n.level == "error"))
}
#[tauri::command]
#[specta::specta]
pub fn get_active_notices(app: AppHandle) -> NoticeSnapshot {
    let store = app.state::<NoticeStore>();
    let mut inner = store.0.lock().unwrap_or_else(|e| e.into_inner());
    let snapshot = NoticeSnapshot { raised: None, ..inner.snapshot.clone() };
    inner.snapshot.open_requested = false;
    snapshot
}
#[tauri::command]
#[specta::specta]
pub fn dismiss_active_notice(app: AppHandle, id: u64) {
    update(&app, |inner| inner.remove(|notice| notice.id == id));
}
#[tauri::command]
#[specta::specta]
pub fn publish_ui_notice(app: AppHandle, source: String, level: String, title: String, detail: Option<String>) {
    if !matches!(level.as_str(), "error" | "warning") { return; }
    raise(&app, &format!("ui:{source}"), None, &level, &title, None, detail, None);
}

#[cfg(test)]
mod tests {
    use super::*;
    fn notice(source: &str) -> Notice {
        Notice { id: 0, source: source.into(), level: "error".into(), title: "failure".into(),
            title_key: None, detail: None, target: None, generation: 0 }
    }
    #[test]
    fn dismissing_old_row_does_not_remove_same_error_raised_again() {
        let mut inner = Inner::default();
        inner.raise(notice("dictation"), None);
        let old_id = inner.snapshot.notices[0].id;
        inner.raise(notice("dictation"), None);
        assert!(!inner.remove(|n| n.id == old_id));
        assert_eq!(inner.snapshot.notices.len(), 1);
    }
    #[test]
    fn old_run_cannot_clear_or_replace_newer_failure() {
        let mut inner = Inner::default();
        inner.begin("file", 1);
        inner.begin("file", 2);
        inner.raise(notice("file"), Some(2));
        assert!(!inner.succeed("file", 1));
        assert!(!inner.raise(notice("file"), Some(1)));
        assert_eq!(inner.snapshot.notices.len(), 1);
    }
    #[test]
    fn success_clears_old_problem_but_preserves_current_warning() {
        let mut inner = Inner::default();
        inner.raise(notice("model"), None);
        inner.raise(notice("file"), None);
        inner.begin("model", 1);
        assert!(inner.succeed("model", 1));
        assert_eq!(inner.snapshot.notices[0].source, "file");
        inner.begin("file", 1);
        inner.raise(notice("file"), Some(1));
        assert!(!inner.succeed("file", 1));
    }
}
