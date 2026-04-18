use tauri::AppHandle;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FinalTextOutputSource {
    Dictation,
}

pub struct FinalTextOutput<'a> {
    pub source: FinalTextOutputSource,
    pub text: &'a str,
}

pub fn before_final_text_output(app: &AppHandle, output: FinalTextOutput<'_>) {
    match output.source {
        FinalTextOutputSource::Dictation => {
            crate::settings::record_dictation_stats_for_text(app, output.text);
        }
    }
}
