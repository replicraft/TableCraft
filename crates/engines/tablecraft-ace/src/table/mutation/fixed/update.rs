pub use super::super::requests::FixedScalarUpdate;
use super::super::{physical::FixedAssignment, rows::RowAssignment};

pub(in crate::table::mutation) struct EncodedUpdate<const WIDTH: usize> {
    pub row: usize,
    pub expected: [u8; WIDTH],
    pub value: [u8; WIDTH],
}
impl<const WIDTH: usize> RowAssignment for EncodedUpdate<WIDTH> {
    fn row(&self) -> usize {
        self.row
    }
}
impl<const WIDTH: usize> FixedAssignment<WIDTH> for EncodedUpdate<WIDTH> {
    fn expected(&self) -> [u8; WIDTH] {
        self.expected
    }
    fn value(&self) -> [u8; WIDTH] {
        self.value
    }
}
