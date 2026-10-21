use super::Expr;
use tablecraft_sql::select::BinaryOperator;

pub(crate) struct ParentFilter {
    // Sorted local metadata positions, or original relation-field keys for joined/mixed scans.
    pub collections: Vec<usize>,
    pub columns: Vec<tablecraft_schema::Column>,
    pub predicate: ParentPredicate,
    // Joined/mixed plans use stable leaf ordinals/local metadata positions. Collection
    // keys then denote original relation fields, not scalar row carriers.
    pub locations: Option<Vec<ParentLocation>>,
}
pub(crate) struct ParentLocation {
    pub leaf: usize,
    pub member: usize,
}
pub(crate) enum ParentPredicate {
    Scalar(Expr),
    // CollectionItem(0) denotes the current item; nested scopes retain its depth.
    Any {
        collection: usize,
        predicate: Expr,
        capture: bool,
    },
    Not(Box<ParentPredicate>),
    Logical(BinaryOperator, Box<ParentPredicate>, Box<ParentPredicate>),
}
