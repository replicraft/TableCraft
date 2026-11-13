mod attribute;
mod columns;
mod metadata;
mod read;
mod record;
pub use attribute::QueryAttribute;
pub use metadata::{SavedQueryDefinition, SavedQueryRow};
pub use read::read_saved_query_definition;

#[cfg(feature = "journal")]
mod expression_edit;
#[cfg(feature = "journal")]
pub use expression_edit::{
    NativeQueryExpressionEditsPlan, OwnedQueryFragmentExpressionEdit, QueryExpressionEditsError,
    QueryFragmentExpressionEdit,
};

#[cfg(feature = "journal")]
mod fitting;
#[cfg(feature = "journal")]
pub use fitting::{NativeQueryExpressionPlan, QueryExpressionError};

#[cfg(all(feature = "journal", feature = "ace16-create"))]
pub(crate) mod creation;
#[cfg(all(feature = "journal", feature = "ace16-create"))]
pub use creation::{
    NativeQueryCreationCandidate, NativeQueryStorageChange, QueryCreationDefinition,
    QueryCreationError,
};
