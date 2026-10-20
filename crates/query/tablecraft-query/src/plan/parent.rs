use super::Expr;
use tablecraft_sql::select::BinaryOperator;

pub(crate) struct ParentFilter {
    // Sorted positions in the source's complete multivalue metadata.
    pub collections: Vec<usize>,
    pub predicate: ParentPredicate,
}
pub(crate) enum ParentPredicate {
    Scalar(Expr),
    // Output(0) inside this expression denotes one borrowed collection item.
    Any { collection: usize, predicate: Expr },
    Not(Box<ParentPredicate>),
    Logical(BinaryOperator, Box<ParentPredicate>, Box<ParentPredicate>),
}
