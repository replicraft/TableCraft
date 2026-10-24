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
pub(crate) struct ParentMember {
    pub collection: usize,
    pub group: usize,
}
pub(crate) struct ParentProduct {
    pub members: Vec<ParentMember>,
    pub groups: usize,
    pub predicate: Expr,
    pub capture: bool,
}
pub(crate) enum ParentPredicate {
    Scalar(Expr),
    Empty {
        collection: usize,
        negated: bool,
    },
    // Item references retain their tuple position and correlated query-scope depth.
    Any {
        collection: usize,
        predicate: Expr,
        capture: bool,
    },
    AnyMany(Box<ParentProduct>),
    Not(Box<ParentPredicate>),
    Logical(BinaryOperator, Box<ParentPredicate>, Box<ParentPredicate>),
}
