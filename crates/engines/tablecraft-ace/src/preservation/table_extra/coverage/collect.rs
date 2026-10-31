use super::super::{TableExtra, TableExtraCoverage, decode};
use super::{MacroCoverageChild, TableMacroCoverage};
use crate::{
    InspectError, NativeImage, NativeObjectOrigin, NativeTableStorage, PayloadPresence,
    header::check,
    preservation::{payload_data::read::read_selected, payloads::PAYLOAD_FIELDS},
};
use tablecraft_core::{ids::ObjectId, limits::ResourceBudget};

fn origin<'image>(
    image: &'image NativeImage,
    object: ObjectId,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<&'image NativeObjectOrigin, InspectError> {
    let mut selected = None;
    for candidate in image.catalog().origins() {
        check(budget, cancelled)?;
        if candidate.object_id() == object && selected.replace(candidate).is_some() {
            return Err(InspectError::Malformed);
        }
    }
    selected.ok_or(InspectError::UnknownObject)
}

impl NativeImage {
    /// Retain exact table/direct-child catalog and opaque payload observations.
    /// This is explicit inert inspection, never required-event discovery,
    /// complete dependency coverage, an absence certificate or save permission.
    pub fn inspect_table_macro_coverage(
        &self,
        object: ObjectId,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<TableMacroCoverage<'_>, InspectError> {
        let _span = tracing::debug_span!("native.table.macro_coverage.inspect").entered();
        check(budget, cancelled)?;
        let catalog = self.catalog().catalog();
        let selected = origin(self, object, budget, cancelled)?;
        if selected.table_storage() != Some(NativeTableStorage::Local) {
            return Err(InspectError::Unsupported);
        }

        // Descriptor and origin rows are created together by catalog capture.
        // Still check their exact agreement rather than inferring containment
        // from one raw ID. Native ParentId 0 is the no-parent sentinel.
        if catalog.objects().len() != self.catalog().origins().len() {
            return Err(InspectError::Malformed);
        }
        let mut count = 0_usize;
        for (candidate, native) in catalog.objects().iter().zip(self.catalog().origins()) {
            check(budget, cancelled)?;
            if candidate.id() != native.object_id() {
                return Err(InspectError::Malformed);
            }
            let semantic_child = candidate.parent() == Some(object);
            let native_child =
                native.parent_id() != 0 && native.parent_id() == selected.native_id();
            if semantic_child != native_child {
                return Err(InspectError::Malformed);
            }
            if semantic_child {
                if candidate.id() == object {
                    return Err(InspectError::Malformed);
                }
                count = count.checked_add(1).ok_or(InspectError::StructureLimit)?;
            }
        }
        budget.charge_allocation(count as u64, size_of::<MacroCoverageChild<'_>>() as u64)?;
        let mut children = Vec::new();
        children
            .try_reserve_exact(count)
            .map_err(|_| InspectError::Allocation)?;

        // Each source payload is assembled exactly once. Root Extra moves into
        // the existing MR2 model; child streams remain opaque without guessing
        // their format from XML-looking bytes or unknown object type codes.
        let [body, module, extra] = read_selected(
            self,
            object,
            [true; PAYLOAD_FIELDS.len()],
            budget,
            cancelled,
        )?;
        let presence = extra.presence;
        let decoded = match extra.bytes.as_deref() {
            Some(raw) => decode::decode(raw, budget, cancelled)?,
            None if presence == PayloadPresence::Present => return Err(InspectError::Malformed),
            None => decode::Decoded {
                coverage: TableExtraCoverage::NotPresent,
                names: Vec::new(),
                groups: Vec::new(),
            },
        };
        let extra = TableExtra {
            object,
            native_id: selected.native_id(),
            parent_id: selected.parent_id(),
            catalog_page: selected.page(),
            catalog_slot: selected.slot(),
            presence,
            raw: extra.bytes,
            decoded,
        };
        for (candidate, native) in catalog.objects().iter().zip(self.catalog().origins()) {
            check(budget, cancelled)?;
            if candidate.parent() != Some(object) {
                continue;
            }
            let payloads = self.read_object_payloads(candidate.id(), budget, cancelled)?;
            children.push(MacroCoverageChild {
                object: candidate.id(),
                descriptor: candidate,
                origin: native,
                payloads,
            });
        }
        check(budget, cancelled)?;
        Ok(TableMacroCoverage {
            image: self,
            origin: selected,
            body,
            module,
            extra,
            children,
        })
    }
}
