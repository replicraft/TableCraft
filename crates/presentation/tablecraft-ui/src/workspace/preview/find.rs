//! Synthetic Find controls never run a native search or manufacture a match.
use super::Preview;
use crate::FindRequest;

impl Preview {
    pub(super) fn prepare_find_scene(&mut self, searching: bool) {
        let request = searching.then(|| self.request());
        self.state.prepare_mock_find(request);
    }

    pub(super) fn mock_find(&mut self, request: FindRequest) {
        let request_id = self.request();
        self.state.begin_find(request_id, request);
        self.state
            .fail_find(request_id, self.state.localizer.text("find-mock-help"));
    }
}
