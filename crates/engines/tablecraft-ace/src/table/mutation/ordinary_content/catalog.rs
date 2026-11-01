//! Complete physical catalog rows and their closed native object graph.
use super::{definition, ledger, records, refused};
use crate::{
    EditError, InspectError, NativeImage,
    format::{
        bytes::{u16_at, u32_at},
        page::PAGE_BYTES,
        text,
    },
    header::check,
    limits::MAX_LIVE_ROWS,
};
use tablecraft_core::{
    ids::ObjectId,
    limits::{Resource, ResourceBudget},
    values::LegacyDateTime,
};
use tablecraft_model::ObjectKind;

const CATALOG_DEFINITION: u32 = 2;
const FIELD_COUNT: usize = 15;
const VARIABLE_COUNT: usize = 9;
const FIXED_BYTES: usize = 32;
const FOOTER_BYTES: usize = (VARIABLE_COUNT + 2) * size_of::<u16>() + 2;
const MASK: [u8; 2] = [0x1f, 0x60];
const LOCAL_TABLE: u16 = 1;
const CONTAINER: u16 = 3;
const SYSTEM_FLAGS: u32 = 0x8000_0002;
const ID_OFFSET: usize = 2;
const PARENT_OFFSET: usize = 6;
const TYPE_OFFSET: usize = 10;
const FLAGS_OFFSET: usize = 12;
const CREATED_OFFSET: usize = 16;
const UPDATED_OFFSET: usize = 24;
const MAX_NAME_BYTES: usize = 510;
const CONTAINER_COUNT: usize = 4;
const SYSTEM_TABLE_COUNT: usize = 3;

pub(super) struct Objects {
    pub systems: Vec<(u32, definition::Role)>,
    pub users: Vec<(ObjectId, u32)>,
}

struct Object {
    id: u32,
    parent: u32,
    kind: u16,
    flags: u32,
    name: String,
    object: ObjectId,
}

pub(super) fn capture<'image>(
    image: &'image NativeImage,
    ledger: &mut ledger::Ledger<'image>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Objects, EditError> {
    let _span = tracing::debug_span!("native.ordinary_content.catalog").entered();
    let table = definition::capture(
        image,
        ledger,
        CATALOG_DEFINITION,
        definition::Role::Catalog,
        budget,
        cancelled,
    )?;
    if table.layout.max_columns != FIELD_COUNT || table.layout.variable_slots != VARIABLE_COUNT {
        return Err(refused());
    }
    let count = usize::try_from(table.layout.records)
        .ok()
        .filter(|count| *count <= MAX_LIVE_ROWS)
        .ok_or_else(refused)?;
    let mut objects: Vec<Object> = vector(count, budget)?;
    for number in ledger.members(table.owned, budget, cancelled)? {
        ledger.claim(number, ledger::Role::Data, budget, cancelled)?;
        let page = ledger.page(number)?;
        for slot in 0..records::count(page, CATALOG_DEFINITION, budget, cancelled)? {
            check(budget, cancelled)?;
            if objects.len() >= count {
                return Err(refused());
            }
            budget
                .charge(Resource::Rows, 1)
                .map_err(InspectError::from)?;
            let object = decode(image, records::row(page, slot)?, budget, cancelled)?;
            bind_inventory(
                image,
                &object,
                number,
                slot,
                objects.len(),
                budget,
                cancelled,
            )?;
            for previous in &objects {
                check(budget, cancelled)?;
                if previous.id == object.id {
                    return Err(refused());
                }
                if previous.kind == LOCAL_TABLE && object.kind == LOCAL_TABLE {
                    name_work(&previous.name, &object.name, budget)?;
                    if previous.name.eq_ignore_ascii_case(&object.name) {
                        return Err(refused());
                    }
                }
            }
            objects.push(object);
        }
    }
    if objects.len() != count
        || count != image.catalog().origins().len()
        || count != image.catalog().catalog().objects().len()
    {
        return Err(refused());
    }
    classify(image, &objects, budget, cancelled)
}

fn decode(
    image: &NativeImage,
    row: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Object, EditError> {
    check(budget, cancelled)?;
    budget
        .charge(Resource::Steps, row.len() as u64)
        .map_err(InspectError::from)?;
    if row.len() < FIXED_BYTES + FOOTER_BYTES || usize::from(u16_at(row, 0)?) != FIELD_COUNT {
        return Err(refused());
    }
    let variables = row.len() - MASK.len() - size_of::<u16>();
    let footer = row.len() - FOOTER_BYTES;
    if row[row.len() - MASK.len()..] != MASK
        || usize::from(u16_at(row, variables)?) != VARIABLE_COUNT
        || usize::from(u16_at(row, footer)?) != footer
    {
        return Err(refused());
    }
    for slot in 0..VARIABLE_COUNT {
        check(budget, cancelled)?;
        let at = variables - (slot + 1) * size_of::<u16>();
        let expected = if slot == 0 { FIXED_BYTES } else { footer };
        if usize::from(u16_at(row, at)?) != expected {
            return Err(refused());
        }
    }
    for at in [CREATED_OFFSET, UPDATED_OFFSET] {
        let bits = u64::from_le_bytes(row[at..at + size_of::<u64>()].try_into().unwrap());
        LegacyDateTime::from_bits(bits).map_err(|_| refused())?;
    }
    let encoded = &row[FIXED_BYTES..footer];
    if encoded.is_empty() || encoded.len() > MAX_NAME_BYTES {
        return Err(refused());
    }
    let name = text::decode(encoded, true, budget)?;
    budget
        .charge(Resource::Steps, (name.len() * 2) as u64)
        .map_err(InspectError::from)?;
    if name.is_empty()
        || name.chars().any(char::is_control)
        || name.encode_utf16().count() > MAX_NAME_BYTES / 2
    {
        return Err(refused());
    }
    let id = u32_at(row, ID_OFFSET)?;
    if id == 0 {
        return Err(refused());
    }
    check(budget, cancelled)?;
    Ok(Object {
        id,
        parent: u32_at(row, PARENT_OFFSET)?,
        kind: u16_at(row, TYPE_OFFSET)?,
        flags: u32_at(row, FLAGS_OFFSET)?,
        name,
        object: image.catalog().catalog().scope().object_id(u64::from(id)),
    })
}

fn bind_inventory(
    image: &NativeImage,
    object: &Object,
    page: u32,
    slot: usize,
    ordinal: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    check(budget, cancelled)?;
    let origin = image.catalog().origins().get(ordinal).ok_or_else(refused)?;
    let descriptor = image
        .catalog()
        .catalog()
        .objects()
        .get(ordinal)
        .ok_or_else(refused)?;
    let kind = match object.kind {
        LOCAL_TABLE => ObjectKind::Table,
        CONTAINER => ObjectKind::Container,
        _ => return Err(refused()),
    };
    let parent = (object.parent != 0).then(|| {
        image
            .catalog()
            .catalog()
            .scope()
            .object_id(u64::from(object.parent))
    });
    name_work(&object.name, descriptor.name().as_str(), budget)?;
    if origin.object_id() != object.object
        || origin.native_id() as u32 != object.id
        || origin.parent_id() as u32 != object.parent
        || origin.type_code() as u16 != object.kind
        || origin.flags() != object.flags
        || origin.page() != page
        || usize::from(origin.slot()) != slot
        || descriptor.id() != object.object
        || descriptor.parent() != parent
        || descriptor.kind() != kind
        || descriptor.name().as_str() != object.name
    {
        return Err(refused());
    }
    Ok(())
}

fn classify(
    image: &NativeImage,
    objects: &[Object],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Objects, EditError> {
    let root = container(objects, "DatabaseRoot", budget, cancelled)?;
    let tables = container(objects, "Tables", budget, cancelled)?;
    let databases = container(objects, "Databases", budget, cancelled)?;
    let relationships = container(objects, "Relationships", budget, cancelled)?;
    if root.parent != 0
        || [tables, databases, relationships]
            .iter()
            .any(|item| item.parent != root.id)
    {
        return Err(refused());
    }
    let mut systems = vector(SYSTEM_TABLE_COUNT - 1, budget)?;
    let mut users = vector(objects.len(), budget)?;
    let mut seen = [false; SYSTEM_TABLE_COUNT];
    for object in objects {
        check(budget, cancelled)?;
        if object.kind == CONTAINER {
            if ![root.id, tables.id, databases.id, relationships.id].contains(&object.id) {
                return Err(refused());
            }
            continue;
        }
        if object.kind != LOCAL_TABLE
            || object.parent != tables.id
            || object.id as usize >= image.as_bytes().len() / PAGE_BYTES
        {
            return Err(refused());
        }
        if object.flags == 0 {
            users.push((object.object, object.id));
            continue;
        }
        if object.flags != SYSTEM_FLAGS {
            return Err(refused());
        }
        let (index, role) = match object.name.as_str() {
            "MSysObjects" if object.id == CATALOG_DEFINITION => (0, definition::Role::Catalog),
            "MSysRelationships" => (1, definition::Role::Relationships),
            "MSysACEs" => (2, definition::Role::Aces),
            _ => return Err(refused()),
        };
        if seen[index] {
            return Err(refused());
        }
        seen[index] = true;
        if role != definition::Role::Catalog {
            systems.push((object.id, role));
        }
    }
    if !seen.into_iter().all(|found| found)
        || objects.len() != CONTAINER_COUNT + SYSTEM_TABLE_COUNT + users.len()
    {
        return Err(refused());
    }
    Ok(Objects { systems, users })
}

fn container<'a>(
    objects: &'a [Object],
    name: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<&'a Object, EditError> {
    let mut selected = None;
    for object in objects {
        check(budget, cancelled)?;
        name_work(&object.name, name, budget)?;
        if object.kind == CONTAINER
            && object.name == name
            && (object.flags != SYSTEM_FLAGS || selected.replace(object).is_some())
        {
            return Err(refused());
        }
    }
    selected.ok_or_else(refused)
}

fn vector<T>(count: usize, budget: &mut ResourceBudget) -> Result<Vec<T>, EditError> {
    budget
        .charge_allocation(count as u64, size_of::<T>() as u64)
        .map_err(InspectError::from)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;
    Ok(result)
}

fn name_work(left: &str, right: &str, budget: &mut ResourceBudget) -> Result<(), EditError> {
    budget
        .charge(Resource::Steps, left.len().max(right.len()) as u64)
        .map_err(InspectError::from)?;
    Ok(())
}
