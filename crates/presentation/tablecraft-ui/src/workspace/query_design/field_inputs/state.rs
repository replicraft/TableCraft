//! The draft owner reveals an unresolved field without executing or discarding it.
impl super::super::QueryDesign {
    pub(in crate::workspace) fn clear_field_input_hints(&mut self) {
        if let Some(session) = &mut self.session {
            session.field_inputs.clear_hints();
        }
    }

    pub(in crate::workspace) fn has_unresolved_field_inputs(&self) -> bool {
        self.session
            .as_ref()
            .is_some_and(|session| session.field_inputs.unresolved())
    }

    pub(in crate::workspace) fn reveal_unresolved_input(&mut self, reason: String) -> bool {
        let Some(session) = self.session.as_mut() else {
            return false;
        };
        let Some(index) = session.field_inputs.first_unresolved() else {
            return false;
        };
        session.field_inputs.reveal = Some(index);
        session.error = Some(reason);
        session.mode = super::super::Mode::Design;
        true
    }
}
