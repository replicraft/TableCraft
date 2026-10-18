//! Shared application-object catalog metadata, independent of native codecs/UI.
//!
//! ```
//! use tablecraft_model::{CatalogScope, ObjectCatalog, ObjectDescriptor, ObjectKind};
//! use tablecraft_schema::{NameComparison, SchemaName};
//! let scope = CatalogScope::new(7); // Caller owns namespace uniqueness.
//! let table = ObjectDescriptor::new(scope.object_id(1),
//!     SchemaName::new("Orders".into())?, ObjectKind::Table, None);
//! let catalog = ObjectCatalog::new(scope, vec![table], NameComparison::Exact, 100)?;
//! assert_eq!(catalog.lookup("Orders", Some(ObjectKind::Table), None)?
//!     .map(|o| o.id()), Some(scope.object_id(1)));
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
#![deny(missing_docs)]
mod catalog;
mod error;
mod identity;
mod object;
pub use catalog::ObjectCatalog;
pub use error::ModelError;
pub use identity::CatalogScope;
pub use object::{ObjectDescriptor, ObjectKind};
