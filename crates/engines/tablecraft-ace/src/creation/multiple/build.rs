//! One complete image with explicit roles and independently bound table records.
use super::super::{
    CreateDatabaseError, build, definition,
    profile::{self, Fields},
    record::{self, DataPage},
};
use super::{layout, request::Captured};
use crate::{format::row, header::check};
use tablecraft_core::limits::ResourceBudget;

pub(in crate::creation) fn build(
    captured: &Captured<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, CreateDatabaseError> {
    let mut bytes = build::initialize(captured.layout.image_bytes, budget, cancelled)?;
    {
        let mut maps = DataPage::new(build::page(&mut bytes, profile::MAP_PAGE)?, 0)?;
        for slot in 0..captured.layout.map_rows {
            check(budget, cancelled)?;
            let member = member(captured, slot, budget, cancelled)?;
            record::inline_map(
                maps.append(profile::MAP_RECORD_BYTES, budget, cancelled)?,
                member,
            )?;
        }
        maps.finish()?;
    }
    for (page, fields, rows, owned, candidate, catalog) in [
        (
            profile::CATALOG_DEFINITION,
            Fields::new(&profile::CATALOG_FIELDS),
            captured.layout.catalog_rows,
            profile::CATALOG_MAP_SLOT,
            profile::CATALOG_CANDIDATE_SLOT,
            true,
        ),
        (
            profile::RELATIONSHIPS_DEFINITION,
            Fields::new(&profile::RELATIONSHIP_FIELDS),
            0,
            profile::RELATIONSHIPS_MAP_SLOT,
            profile::RELATIONSHIPS_MAP_SLOT + profile::TABLE_COUNT,
            false,
        ),
        (
            profile::ACES_DEFINITION,
            Fields::new(&profile::ACE_FIELDS),
            0,
            profile::ACES_MAP_SLOT,
            profile::ACES_MAP_SLOT + profile::TABLE_COUNT,
            false,
        ),
    ] {
        check(budget, cancelled)?;
        let owned = u8::try_from(owned).map_err(|_| CreateDatabaseError::Layout)?;
        let candidate = u8::try_from(candidate).map_err(|_| CreateDatabaseError::Layout)?;
        definition::write_role(
            build::page(&mut bytes, page)?,
            fields,
            rows,
            definition::Role {
                user: false,
                catalog,
                owned: row::reference(profile::MAP_PAGE as u32, owned),
                candidate: row::reference(profile::MAP_PAGE as u32, candidate),
            },
            budget,
            cancelled,
        )?;
    }
    for (index, request) in captured.tables.iter().enumerate() {
        check(budget, cancelled)?;
        let role = captured.layout.role(index)?;
        let index_role = captured.layout.index_role(index)?;
        let primary = match (request.primary.as_ref(), index_role) {
            (Some(prepared), Some(index)) => Some(definition::PrimaryRole {
                prepared,
                root: index.root,
                map: index.owned_reference()?,
            }),
            (None, None) => None,
            _ => return Err(CreateDatabaseError::Layout),
        };
        definition::write_role_with_primary(
            build::page(&mut bytes, role.definition)?,
            request.fields(),
            request.rows.count(),
            definition::Role {
                user: true,
                catalog: false,
                owned: role.owned_reference()?,
                candidate: role.candidate_reference()?,
            },
            primary,
            budget,
            cancelled,
        )?;
    }
    {
        let mut catalog = DataPage::new(
            build::page(&mut bytes, profile::CATALOG_DATA)?,
            profile::CATALOG_DEFINITION,
        )?;
        for object in build::systems() {
            record::catalog(
                catalog.append(record::catalog_length(object.name), budget, cancelled)?,
                &object,
                budget,
                cancelled,
            )?;
        }
        for (index, request) in captured.tables.iter().enumerate() {
            check(budget, cancelled)?;
            let role = captured.layout.role(index)?;
            let object = profile::CatalogObject {
                id: u32::try_from(role.definition).map_err(|_| CreateDatabaseError::Layout)?,
                parent: profile::TABLES_ID,
                name: request.catalog_name,
                system: false,
            };
            let properties = request.properties.as_ref();
            let length = record::catalog_length_with_properties(object.name, properties)?;
            record::catalog_with_properties(
                catalog.append(length, budget, cancelled)?,
                &object,
                properties,
                budget,
                cancelled,
            )?;
        }
        catalog.finish()?;
    }
    for (index, request) in captured.tables.iter().enumerate() {
        let role = captured.layout.role(index)?;
        let mut data = DataPage::new(build::page(&mut bytes, role.data)?, role.definition)?;
        for ordinal in 0..request.rows.count() {
            record::user(
                data.append(request.row_length(ordinal), budget, cancelled)?,
                request,
                ordinal,
                budget,
                cancelled,
            )?;
        }
        data.finish()?;
    }
    for (index, request) in captured.tables.iter().enumerate() {
        check(budget, cancelled)?;
        if let Some(primary) = request.primary.as_ref() {
            let role = captured.layout.role(index)?;
            let index_role = captured
                .layout
                .index_role(index)?
                .ok_or(CreateDatabaseError::Layout)?;
            primary.write_leaf(
                build::page(&mut bytes, index_role.root)?,
                role.definition,
                role.data,
                budget,
                cancelled,
            )?;
        }
    }
    check(budget, cancelled)?;
    Ok(bytes)
}

fn member(
    captured: &Captured<'_>,
    slot: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Option<usize>, CreateDatabaseError> {
    if slot >= captured.layout.index_maps_start() {
        for index in 0..captured.tables.len() {
            check(budget, cancelled)?;
            if let Some(role) = captured.layout.index_role(index)?
                && role.owned == slot
            {
                return Ok(Some(role.root));
            }
        }
        return Err(CreateDatabaseError::Layout);
    }
    if matches!(
        slot,
        profile::CATALOG_MAP_SLOT | profile::CATALOG_CANDIDATE_SLOT
    ) {
        return Ok(Some(profile::CATALOG_DATA));
    }
    // System-only creation retains the native map slots but grants no user
    // membership or candidate page through either vacant user reservation.
    if captured.tables.is_empty() {
        return Ok(None);
    }
    let (index, owned) = if slot == profile::USER_MAP_SLOT {
        (0, true)
    } else if slot == profile::USER_CANDIDATE_SLOT {
        (0, false)
    } else if slot >= profile::MAP_ROWS {
        let offset = slot - profile::MAP_ROWS;
        (
            1 + offset / layout::APPENDED_MAPS,
            offset.is_multiple_of(layout::APPENDED_MAPS),
        )
    } else {
        return Ok(None);
    };
    let role: layout::Role = captured.layout.role(index)?;
    let request = captured
        .tables
        .get(index)
        .ok_or(CreateDatabaseError::Layout)?;
    let fits = request.free_bytes >= request.minimum_row_bytes + row::SLOT_BYTES;
    Ok((owned || fits).then_some(role.data))
}
