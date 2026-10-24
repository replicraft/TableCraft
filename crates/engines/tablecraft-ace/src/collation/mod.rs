//! Native text-order metadata, separate from decoded string comparison.
mod comparison;
mod key;
mod like_units;
mod search;
mod sort_order;
use crate::{InspectError, format::header_mask, header::check};
pub use comparison::{
    compare_general_like_units, compare_general_text, compare_general_utf16,
    general_like_literal_step,
};
pub use key::{NativeTextKey, encode_text_index_key, encode_utf16_text_index_key};
pub use like_units::{general_like_unit_step, is_general_like_unit};
pub use search::{GeneralTextSearch, find_general_text, find_general_utf16};
pub use sort_order::{GeneralSortFamily, NativeSortOrder};
use std::io::{Read, Seek};
use tablecraft_core::limits::{Resource, ResourceBudget};
use zeroize::Zeroizing;

const DATABASE_SORT_OFFSET: usize = 110;

/// Read the stored Jet 4 / ACE database text order from masked page zero.
/// No credentials, record reads or active content are required. Unknown values
/// remain metadata, not a default collation or permission to use text indexes.
pub fn inspect_database_sort_order<R: Read + Seek>(
    source: &mut R,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<NativeSortOrder, InspectError> {
    let _span = tracing::debug_span!("native.text.sort_order").entered();
    let header = crate::inspect_header(source, budget, cancelled)?;
    // Encryption protects data pages; masked page-zero metadata remains inert.
    header.decoded().read_profile()?;
    budget.charge(Resource::Bytes, header_mask::PREFIX_END as u64)?;
    let mut prefix = Zeroizing::new([0; header_mask::PREFIX_END]);
    tablecraft_io::read_exact_at(source, 0, &mut *prefix, budget, cancelled)?;
    header_mask::apply(&mut *prefix)?;
    let order = NativeSortOrder::read(&prefix[DATABASE_SORT_OFFSET..])?;
    check(budget, cancelled)?;
    Ok(order)
}
