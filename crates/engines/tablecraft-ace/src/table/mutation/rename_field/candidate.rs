use super::{FieldRenameError as Error, NativeFieldRenameIdentity, source, verify};
use crate::{InspectError, NativeImage, header::check};
use tablecraft_core::limits::{Resource, ResourceBudget};

const COPY_BYTES: usize = 16 * 1024;

/// Owning unpublished field rename with exact original and candidate images.
/// No journal, native save, dependency repair or later ADD admission is supplied.
pub struct NativeFieldRenameCandidate {
    pub(super) source: source::Qualified,
    after: Vec<u8>,
    new_name: String,
}

impl NativeFieldRenameCandidate {
    /// Complete source-qualified original, including all historical bodies.
    pub fn before_bytes(&self) -> &[u8] {
        self.source.image().as_bytes()
    }

    /// Complete candidate; only the two parser-owned name spans can differ.
    pub fn after_bytes(&self) -> &[u8] {
        &self.after
    }

    /// Actual original table spelling.
    pub fn table(&self) -> &str {
        self.source.observed.schema.name()
    }

    /// Actual original native field name, independent of a displayed Caption.
    pub fn column(&self) -> &str {
        self.source.observed.column().name()
    }

    /// Exact requested replacement spelling.
    pub fn new_name(&self) -> &str {
        &self.new_name
    }

    /// Original native table/column identity captured by the real parsers.
    pub const fn identity(&self) -> NativeFieldRenameIdentity {
        self.source.observed.identity
    }

    /// Exact-spelling no-ops still qualify the complete original source.
    pub fn changed(&self) -> bool {
        self.column() != self.new_name
    }
}

impl NativeImage {
    /// Observe one actual field's eligibility without a fake proposed name.
    /// This is read-only metadata/source evidence, never a rename capability.
    pub fn inspect_same_width_field_rename_source(
        &self,
        table: &str,
        column: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeFieldRenameIdentity, Error> {
        self.check_field_history_name_auto_correct_off_source(budget, cancelled)?;
        Ok(source::Observed::capture(self, table, column, budget, cancelled)?.identity)
    }

    /// Prepare a separate same-width non-key Number/Short Text field rename.
    /// The complete settled OFF source is proved once and retained privately;
    /// names come from the real TDEF and live MR2 parsers, not byte searching.
    pub fn prepare_same_width_field_rename(
        self,
        table: &str,
        column: &str,
        new_name: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeFieldRenameCandidate, Error> {
        let _span = tracing::debug_span!("native.field_rename.prepare_same_width").entered();
        check(budget, cancelled)?;
        if !crate::identifier::valid_name(new_name, budget, cancelled)? {
            return Err(Error::InvalidName);
        }
        let source = source::Qualified::capture(self, table, column, budget, cancelled)?;
        let old_name = source.observed.column().name();
        if !new_name.is_ascii() || new_name.len() != old_name.len() {
            return Err(Error::NameShape);
        }
        for other in source.observed.schema.columns() {
            check(budget, cancelled)?;
            budget
                .charge(
                    Resource::Steps,
                    other.name().len().max(new_name.len()) as u64,
                )
                .map_err(InspectError::from)?;
            if other.number() != source.observed.identity.column_number()
                && other.name().eq_ignore_ascii_case(new_name)
            {
                return Err(Error::DuplicateName);
            }
        }
        budget
            .charge_allocation(new_name.len() as u64, 1)
            .map_err(InspectError::from)?;
        let mut replacement = String::new();
        replacement
            .try_reserve_exact(new_name.len())
            .map_err(|_| InspectError::Allocation)?;
        replacement.push_str(new_name);
        let length = source.image().as_bytes().len();
        budget
            .charge_allocation(length as u64, 1)
            .map_err(InspectError::from)?;
        let mut after = Vec::new();
        after
            .try_reserve_exact(length)
            .map_err(|_| InspectError::Allocation)?;
        for chunk in source.image().as_bytes().chunks(COPY_BYTES) {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, chunk.len() as u64)
                .map_err(InspectError::from)?;
            after.extend_from_slice(chunk);
        }
        for span in [
            &source.observed.definition_name,
            &source.observed.property_name,
        ] {
            check(budget, cancelled)?;
            let bytes = after.get_mut(span.clone()).ok_or(Error::Preservation)?;
            if bytes.len() != replacement.len() * size_of::<u16>() {
                return Err(Error::Preservation);
            }
            budget
                .charge(Resource::Steps, bytes.len() as u64)
                .map_err(InspectError::from)?;
            for (destination, unit) in bytes
                .chunks_exact_mut(size_of::<u16>())
                .zip(replacement.encode_utf16())
            {
                destination.copy_from_slice(&unit.to_le_bytes());
            }
        }
        verify::candidate(&source, &after, &replacement, budget, cancelled)?;
        Ok(NativeFieldRenameCandidate {
            source,
            after,
            new_name: replacement,
        })
    }
}
