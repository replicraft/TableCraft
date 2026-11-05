//! Literal mock service diagnoses exercise existing failure publication only.
use super::{Preview, Scene};

const SERVICE_REPORT: &str = r#"Synthetic service report; no database was opened.
The requested operation could not complete. The previous complete table or query result remains visible.
Stage: checking the selected source against the captured document revision.
Context: an intentionally lengthy diagnostic exercises bounded text layout, selection, and scrolling.
Object label: Customer reconciliation "North / South"; literal <record> & [field] text.
Detail: repeated explanatory sentences are display data, never instructions to execute a query, reopen a path, or retry an operation.
Diagnostic token: SYNTHETIC_ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789
No partial replacement result was published.
Stored parameter choices and the last complete result retain their original identities.
The failure report does not change editing or collection capabilities.
This final line should be reachable by scrolling the diagnostic region."#;

impl Preview {
    pub(super) fn prepare_long_error(&mut self, scene: Scene) {
        let report = SERVICE_REPORT.to_owned();
        match scene {
            Scene::LongReadError => self.state.error = Some(report),
            Scene::LongOpenError => {
                self.state.path = "synthetic/open-failure.accdb".into();
                self.state.password = "synthetic-preview-only".into();
                self.state.error = Some(report);
                self.state.show_open();
            }
            Scene::LongQueryError => {
                self.prepare_query_error();
                self.state.error = Some(report);
            }
            Scene::LongSavedError => self.prepare_saved_error_message(report),
            _ => unreachable!("only long error scenes use this fixture"),
        }
    }
}

#[cfg(test)]
mod tests;
