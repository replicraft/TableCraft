//! Independent complete-image and request-authority controls for real INSERT.
mod authority;
mod bitmap;
mod child_split;
mod decimal;
mod empty;
mod general_height_one;
mod height_one;
pub(super) mod model;
pub(in crate::table::mutation::insert) mod multiple_indexes;
mod multiple_numeric_indexes;
mod ordinary_rows;
mod recursive_tree;
mod split;
mod values;
