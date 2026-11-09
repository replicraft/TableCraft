//! Resource-free Find controls and explicitly authored synthetic occurrences.
use super::Preview;
use crate::{Cell, FindRequest};

impl Preview {
    pub(super) fn prepare_find_scene(&mut self, searching: bool) {
        let request = searching.then(|| self.request());
        self.state.prepare_mock_find(request);
    }

    pub(super) fn prepare_find_occurrence(&mut self, long: bool) {
        let text = if long {
            format!("{}🦀", "a".repeat(520))
        } else {
            "A🦀 note".to_owned()
        };
        let data = |text: String| {
            let mut data = super::fixtures::page("Customers", 0);
            data.columns = vec!["Notes".into()];
            data.rows = vec![vec![Cell::scalar(text)]];
            data.has_more = false;
            data
        };
        // The displayed page and occurrence receipt retain independent text snapshots; copy for
        // the first owner and move the original into the final snapshot.
        self.state.set_table_data(data(text.clone()));
        let request_id = self.request();
        let start_utf16 = if long { 520 } else { 1 };
        self.state
            .prepare_mock_find_occurrence(request_id, data(text), start_utf16);
    }

    pub(super) fn mock_find(&mut self, request: FindRequest) {
        let request_id = self.request();
        self.state.begin_find(request_id, request);
        self.state
            .fail_find(request_id, self.state.localizer.text("find-mock-help"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preview::Scene;

    #[test]
    fn explicit_synthetic_occurrences_retain_exact_utf16_receipts_without_host_actions() {
        for (scene, start) in [(Scene::FindOccurrence, 1), (Scene::FindLongOccurrence, 520)] {
            let preview = Preview::new(scene);
            let state = preview.state();
            assert!(state.mock_preview);
            let occurrence = state
                .current_find_occurrence()
                .expect("authored match receipt");
            assert_eq!((occurrence.row, occurrence.column), (0, 0));
            assert_eq!(
                (occurrence.start_utf16, occurrence.end_utf16),
                (start, start + 2)
            );
            assert!(state.can_find_next(&state.current_find_request().unwrap()));
            assert_eq!(state.data.as_ref().unwrap().rows.len(), 1);
        }
    }
}
