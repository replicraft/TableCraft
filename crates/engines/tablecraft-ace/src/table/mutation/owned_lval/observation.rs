//! Checked read-only map scopes, separate from writable represented-cell ledgers.
use super::super::storage_scope::Scope;
use crate::{
    EditError, LongValueMapReferences, TableSchema, WriteBlocker,
    format::{
        allocation::{GLOBAL_FREE_REFERENCE, MapRole, RepresentedMap},
        bytes::u32_at,
        definition,
        layout::TABLE_FREE_MAP_OFFSET,
        reader::Reader,
    },
};
use std::io::Cursor;

const INLINE_MAP: u8 = 0;
const INDIRECT_MAP: u8 = 1;

enum HintGrammar {
    Inline,
    #[cfg(all(feature = "journal", feature = "ace16-create"))]
    NativeCreation,
}

/// Full allocated storage and membership observations. This type has no edits,
/// future-cell policy or mutation methods; each source still proves its purpose.
pub(in crate::table::mutation) struct ObservedScopes {
    pub ordinary: Scope,
    pub long_value: Scope,
    pub global: RepresentedMap,
}

impl ObservedScopes {
    pub fn capture(
        reader: &mut Reader<'_, Cursor<&[u8]>>,
        schema: &TableSchema,
        references: LongValueMapReferences,
    ) -> Result<Self, EditError> {
        Self::capture_with(reader, schema, references, HintGrammar::Inline)
    }

    /// Native table creation observes represented indirect candidates without
    /// granting that shape to older row or property mutation factories.
    #[cfg(all(feature = "journal", feature = "ace16-create"))]
    pub fn capture_native_creation(
        reader: &mut Reader<'_, Cursor<&[u8]>>,
        schema: &TableSchema,
        references: LongValueMapReferences,
    ) -> Result<Self, EditError> {
        Self::capture_with(reader, schema, references, HintGrammar::NativeCreation)
    }

    fn capture_with(
        reader: &mut Reader<'_, Cursor<&[u8]>>,
        schema: &TableSchema,
        references: LongValueMapReferences,
        grammar: HintGrammar,
    ) -> Result<Self, EditError> {
        let global = RepresentedMap::read(reader, GLOBAL_FREE_REFERENCE, MapRole::GlobalFree)?;
        global.require_allocated_storage(reader, &global)?;
        global.require_allocated(reader, schema.definition_page())?;
        let metadata = definition::read(reader, schema.definition_page())?;
        let ordinary_hint = u32_at(&metadata, TABLE_FREE_MAP_OFFSET)?;
        let ordinary = Scope::capture(
            reader,
            schema.layout.usage,
            ordinary_hint,
            MapRole::TableFreeSpace,
            &global,
        )?;
        let long_value = Scope::capture(
            reader,
            references.owned(),
            references.candidate(),
            MapRole::LongValueCandidate,
            &global,
        )?;
        let hint_supported = match grammar {
            HintGrammar::Inline => long_value.hint.kind() == INLINE_MAP,
            #[cfg(all(feature = "journal", feature = "ace16-create"))]
            HintGrammar::NativeCreation => {
                matches!(long_value.hint.kind(), INLINE_MAP | INDIRECT_MAP)
            }
        };
        if !matches!(long_value.owned.kind(), INLINE_MAP | INDIRECT_MAP) || !hint_supported {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        for &number in &long_value.pages {
            reader.tick()?;
            reader.work(ordinary.pages.len())?;
            if ordinary.pages.contains(&number) {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }
        }

        Ok(Self {
            ordinary,
            long_value,
            global,
        })
    }
}
