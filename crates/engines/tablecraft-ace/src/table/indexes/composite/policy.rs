use super::model::ComponentValue;
use crate::InspectError;
use crate::table::indexes::key::KeyRow;

pub(in crate::table::indexes) trait KeyComponent: Copy {
    fn descending(self) -> bool;
    fn max_bytes(self) -> usize;
    fn decode<A: ComponentValue>(self, bytes: &[u8]) -> Result<(Option<A>, usize), InspectError>;
    fn value<A: ComponentValue>(
        self,
        row: &(impl KeyRow + ?Sized),
    ) -> Result<Option<A>, InspectError>;
}

pub(in crate::table::indexes) trait ColumnSet {
    type Component: KeyComponent;
    fn components(&self) -> &[Self::Component];
    fn supports<A: ComponentValue>(&self) -> bool;
}
