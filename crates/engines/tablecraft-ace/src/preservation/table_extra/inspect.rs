use super::{TableExtra, TableExtraCoverage, decode};
use crate::{
    InspectError, NativeImage, NativeTableStorage,
    header::check,
    preservation::{
        PayloadPresence,
        payload_data::read::read_selected,
        payloads::{EXTRA_FIELD, PAYLOAD_FIELDS},
    },
};
use tablecraft_core::{ids::ObjectId, limits::ResourceBudget};

impl NativeImage {
    /// Inspect bounded wide-MR2 table LvExtra framing without reading unrelated
    /// Lv/LvModule payloads, parsing XML, or executing application content. Only
    /// an exact captured local-table identity is accepted. Structural coverage
    /// never certifies data-macro absence or permits normal writes.
    pub fn inspect_table_extra(
        &self,
        object: ObjectId,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<TableExtra, InspectError> {
        let _span = tracing::debug_span!("native.table.extra.inspect").entered();
        check(budget, cancelled)?;
        let mut selected = None;
        for origin in self.catalog().origins() {
            check(budget, cancelled)?;
            if origin.object_id() == object {
                if selected.is_some() {
                    return Err(InspectError::Malformed);
                }
                selected = Some(origin);
            }
        }
        let origin = selected.ok_or(InspectError::UnknownObject)?;
        if origin.table_storage() != Some(NativeTableStorage::Local) {
            return Err(InspectError::Unsupported);
        }

        let selected = PAYLOAD_FIELDS.map(|name| name == EXTRA_FIELD);
        let [_, _, extra] = read_selected(self, object, selected, budget, cancelled)?;
        let presence = extra.presence;
        let raw = extra.bytes;
        let decoded = match raw.as_deref() {
            Some(bytes) => decode::decode(bytes, budget, cancelled)?,
            None => {
                if presence == PayloadPresence::Present {
                    return Err(InspectError::Malformed);
                }
                decode::Decoded {
                    coverage: TableExtraCoverage::NotPresent,
                    names: Vec::new(),
                    groups: Vec::new(),
                }
            }
        };
        check(budget, cancelled)?;
        Ok(TableExtra {
            object,
            native_id: origin.native_id(),
            parent_id: origin.parent_id(),
            catalog_page: origin.page(),
            catalog_slot: origin.slot(),
            presence,
            raw,
            decoded,
        })
    }
}
