//! Complete bounded framing and identity preflight before any decoded allocation.
use super::super::row_ordinals::RowOrdinals;
use super::{
    carrier::{self, Borrowed},
    codec,
    grammar::Grammar,
    model::{Family, NativeFittedEditIntent, NativeFittedIntentGroup, NativeFittedUpdates},
    profile::WireProfile,
};
use crate::{
    EditError, FixedScalarUpdate, InspectError, SlotScalarUpdate, header::check,
    transaction::intent::NativeIntentError,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const UPDATE_TOTAL_AT: usize = 0;
const UPDATE_ROW_AT: usize = UPDATE_TOTAL_AT + size_of::<u32>();

fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + size_of::<u32>()].try_into().unwrap())
}

struct Cursor<'a> {
    bytes: &'a [u8],
    at: usize,
    grammar: Grammar,
}

struct Group<'a> {
    name: &'a str,
    count: usize,
    family: Family,
    cells: &'a [u8],
}

struct Update<'a> {
    row: usize,
    expected: Borrowed<'a>,
    value: Borrowed<'a>,
}

impl<'a> Cursor<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8], NativeIntentError> {
        let end = self
            .at
            .checked_add(count)
            .ok_or(NativeIntentError::InvalidRecord)?;
        let result = self
            .bytes
            .get(self.at..end)
            .ok_or(NativeIntentError::InvalidRecord)?;
        self.at = end;
        Ok(result)
    }

    fn name(
        &mut self,
        count: usize,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<&'a str, NativeIntentError> {
        let bytes = self.take(count)?;
        carrier::pay_scan(bytes, budget, cancelled)?;
        let value = std::str::from_utf8(bytes).map_err(|_| NativeIntentError::InvalidRecord)?;
        self.grammar.name(value, budget, cancelled)?;
        Ok(value)
    }

    fn group(
        &mut self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Group<'a>, NativeIntentError> {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, codec::GROUP_HEADER_BYTES as u64)
            .map_err(InspectError::from)?;
        let header = self.take(codec::GROUP_HEADER_BYTES)?;
        let total = u32_at(header, codec::GROUP_TOTAL_AT) as usize;
        let count = u32_at(header, codec::GROUP_COUNT_AT_IN_GROUP) as usize;
        if header[codec::GROUP_PADDING_AT..codec::GROUP_RESERVED_AT]
            .iter()
            .any(|byte| *byte != 0)
            || u32_at(header, codec::GROUP_RESERVED_AT) != 0
        {
            return Err(NativeIntentError::InvalidRecord);
        }
        let family = match header[codec::GROUP_FAMILY_AT] {
            0 => Family::Fixed,
            1 => Family::Variable,
            _ => return Err(NativeIntentError::InvalidRecord),
        };
        if count > self.grammar.assignments() {
            return Err(InspectError::StructureLimit.into());
        }
        let rest = total
            .checked_sub(codec::GROUP_HEADER_BYTES)
            .ok_or(NativeIntentError::InvalidRecord)?;
        let mut body = Cursor {
            bytes: self.take(rest)?,
            at: 0,
            grammar: self.grammar,
        };
        let name = body.name(
            u32_at(header, codec::GROUP_NAME_AT) as usize,
            budget,
            cancelled,
        )?;
        let cells = &body.bytes[body.at..];
        let minimum = count
            .checked_mul(codec::MIN_UPDATE_BYTES)
            .ok_or(InspectError::StructureLimit)?;
        if cells.len() < minimum {
            return Err(NativeIntentError::InvalidRecord);
        }
        Ok(Group {
            name,
            count,
            family,
            cells,
        })
    }

    fn carrier(
        &mut self,
        family: Family,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Borrowed<'a>, NativeIntentError> {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, carrier::HEADER_BYTES as u64)
            .map_err(InspectError::from)?;
        let header = self.take(carrier::HEADER_BYTES)?;
        if header[codec::CARRIER_PADDING_AT..codec::CARRIER_LENGTH_AT]
            .iter()
            .any(|byte| *byte != 0)
        {
            return Err(NativeIntentError::InvalidRecord);
        }
        let length = u32_at(header, codec::CARRIER_LENGTH_AT) as usize;
        self.grammar.parse(
            header[codec::CARRIER_TAG_AT],
            self.take(length)?,
            family,
            budget,
            cancelled,
        )
    }

    fn update(
        &mut self,
        family: Family,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Update<'a>, NativeIntentError> {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, codec::UPDATE_HEADER_BYTES as u64)
            .map_err(InspectError::from)?;
        let header = self.take(codec::UPDATE_HEADER_BYTES)?;
        let total = u32_at(header, UPDATE_TOTAL_AT) as usize;
        if total < codec::MIN_UPDATE_BYTES {
            return Err(NativeIntentError::InvalidRecord);
        }
        let row = u64::from_le_bytes(header[UPDATE_ROW_AT..].try_into().unwrap());
        let row = usize::try_from(row).map_err(|_| InspectError::StructureLimit)?;
        let rest = total
            .checked_sub(codec::UPDATE_HEADER_BYTES)
            .ok_or(NativeIntentError::InvalidRecord)?;
        let mut body = Cursor {
            bytes: self.take(rest)?,
            at: 0,
            grammar: self.grammar,
        };
        let expected = body.carrier(family, budget, cancelled)?;
        let value = body.carrier(family, budget, cancelled)?;
        if body.at != body.bytes.len() {
            return Err(NativeIntentError::InvalidRecord);
        }
        Ok(Update {
            row,
            expected,
            value,
        })
    }
}

struct Header<'a> {
    table: &'a str,
    groups_start: usize,
    groups: usize,
    before_length: u64,
    before_binding: [u8; codec::BINDING_BYTES],
}

fn scan<'a>(
    bytes: &'a [u8],
    profile: WireProfile,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Header<'a>, NativeIntentError> {
    check(budget, cancelled)?;
    let grammar = profile.grammar();
    if bytes.len() < codec::HEADER_BYTES || bytes.len() > grammar.record_bytes() {
        return Err(NativeIntentError::InvalidRecord);
    }
    budget
        .charge(Resource::Steps, codec::HEADER_BYTES as u64)
        .map_err(InspectError::from)?;
    if &bytes[..profile.magic().len()] != profile.magic() {
        return Err(if bytes.starts_with(b"TCNI") {
            NativeIntentError::UnsupportedVersion
        } else {
            NativeIntentError::InvalidRecord
        });
    }
    if u16::from_le_bytes(
        bytes[codec::PROFILE_AT..codec::FLAGS_AT]
            .try_into()
            .unwrap(),
    ) != profile.number()
    {
        return Err(NativeIntentError::UnsupportedProfile);
    }
    if u32_at(bytes, codec::TOTAL_AT) as usize != bytes.len()
        || bytes[codec::FLAGS_AT..codec::BEFORE_LENGTH_AT] != [0; 2]
        || u32_at(bytes, codec::RESERVED_AT) != 0
    {
        return Err(NativeIntentError::InvalidRecord);
    }
    let before_length = u64::from_le_bytes(
        bytes[codec::BEFORE_LENGTH_AT..codec::BINDING_AT]
            .try_into()
            .unwrap(),
    );
    grammar.length(before_length)?;
    let groups = u32_at(bytes, codec::GROUP_COUNT_AT) as usize;
    let assignments = u32_at(bytes, codec::ASSIGNMENT_COUNT_AT) as usize;
    if groups == 0 || groups > grammar.groups() || assignments > grammar.assignments() {
        return Err(InspectError::StructureLimit.into());
    }
    let mut cursor = Cursor {
        bytes,
        at: codec::HEADER_BYTES,
        grammar,
    };
    let table = cursor.name(
        u32_at(bytes, codec::TABLE_LENGTH_AT) as usize,
        budget,
        cancelled,
    )?;
    let groups_start = cursor.at;
    let mut total = 0_usize;
    for position in 0..groups {
        let group = cursor.group(budget, cancelled)?;
        total = total
            .checked_add(group.count)
            .filter(|count| *count <= grammar.assignments())
            .ok_or(InspectError::StructureLimit)?;
        // Revisit only bounded preceding headers/names; no decoded allocation
        // is necessary to reject duplicate column identities before reserve.
        let mut previous = Cursor {
            bytes,
            at: groups_start,
            grammar,
        };
        for _ in 0..position {
            let prior = previous.group(budget, cancelled)?;
            budget
                .charge(
                    Resource::Steps,
                    prior.name.len().max(group.name.len()) as u64,
                )
                .map_err(InspectError::from)?;
            if prior.name.eq_ignore_ascii_case(group.name) {
                return Err(EditError::Conflict.into());
            }
        }
        let mut seen = RowOrdinals::default();
        let mut cells = Cursor {
            bytes: group.cells,
            at: 0,
            grammar,
        };
        for _ in 0..group.count {
            let update = cells.update(group.family, budget, cancelled)?;
            seen.insert(update.row, budget, cancelled)?;
        }
        if cells.at != group.cells.len() {
            return Err(NativeIntentError::InvalidRecord);
        }
    }
    if total != assignments || cursor.at != bytes.len() {
        return Err(NativeIntentError::InvalidRecord);
    }
    check(budget, cancelled)?;
    Ok(Header {
        table,
        groups_start,
        groups,
        before_length,
        before_binding: bytes[codec::BINDING_AT..codec::TABLE_LENGTH_AT]
            .try_into()
            .unwrap(),
    })
}

pub(super) fn decode(
    bytes: &[u8],
    profile: WireProfile,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<NativeFittedEditIntent, NativeIntentError> {
    let _span = tracing::debug_span!("native.fitted_intent.decode").entered();
    let header = scan(bytes, profile, budget, cancelled)?;
    let grammar = profile.grammar();
    budget
        .charge_allocation(
            header.groups as u64,
            size_of::<NativeFittedIntentGroup>() as u64,
        )
        .map_err(InspectError::from)?;
    let mut groups = Vec::new();
    groups
        .try_reserve_exact(header.groups)
        .map_err(|_| InspectError::Allocation)?;
    let mut cursor = Cursor {
        bytes,
        at: header.groups_start,
        grammar,
    };
    for _ in 0..header.groups {
        let group = cursor.group(budget, cancelled)?;
        let mut cells = Cursor {
            bytes: group.cells,
            at: 0,
            grammar,
        };
        let updates = match group.family {
            Family::Fixed => {
                budget
                    .charge_allocation(group.count as u64, size_of::<FixedScalarUpdate>() as u64)
                    .map_err(InspectError::from)?;
                let mut result = Vec::new();
                result
                    .try_reserve_exact(group.count)
                    .map_err(|_| InspectError::Allocation)?;
                for _ in 0..group.count {
                    let update = cells.update(group.family, budget, cancelled)?;
                    result.push(FixedScalarUpdate::new(
                        update.row,
                        carrier::owned(update.expected, budget, cancelled)?,
                        carrier::owned(update.value, budget, cancelled)?,
                    ));
                }
                NativeFittedUpdates::Fixed(result)
            }
            Family::Variable => {
                budget
                    .charge_allocation(group.count as u64, size_of::<SlotScalarUpdate>() as u64)
                    .map_err(InspectError::from)?;
                let mut result = Vec::new();
                result
                    .try_reserve_exact(group.count)
                    .map_err(|_| InspectError::Allocation)?;
                for _ in 0..group.count {
                    let update = cells.update(group.family, budget, cancelled)?;
                    result.push(SlotScalarUpdate::new(
                        update.row,
                        carrier::owned(update.expected, budget, cancelled)?,
                        carrier::owned(update.value, budget, cancelled)?,
                    ));
                }
                NativeFittedUpdates::Variable(result)
            }
        };
        groups.push(NativeFittedIntentGroup::new_with_grammar(
            group.name, updates, grammar, budget, cancelled,
        )?);
    }
    NativeFittedEditIntent::from_decoded_with_grammar(
        header.table,
        groups,
        header.before_length,
        header.before_binding,
        grammar,
        budget,
        cancelled,
    )
}
