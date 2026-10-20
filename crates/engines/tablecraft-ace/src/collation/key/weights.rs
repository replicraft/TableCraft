//! Borrowed normalized Jackcess weight data; provenance/license in data/general.
use crate::{GeneralSortFamily, InspectError};
use zerocopy::{
    FromBytes, Immutable, KnownLayout,
    byteorder::{LittleEndian, U32},
};

const LEGACY: &[u8] = include_bytes!("../../../data/general/general-legacy.bin");
const GENERAL: &[u8] = include_bytes!("../../../data/general/general-2010.bin");
pub(super) const VOWELS: &[u8; 256] = include_bytes!("../../../data/general/kana-vowels.bin");
const FILE_HEADER_BYTES: usize = 8;
const UTF16_UNITS: usize = u16::MAX as usize + 1;
const INDEX_BYTES: usize = UTF16_UNITS * size_of::<u32>();
const RECORDS_OFFSET: usize = FILE_HEADER_BYTES + INDEX_BYTES;

pub(super) const SIMPLE: u8 = 0;
pub(super) const REPEAT: u8 = 8;

#[derive(FromBytes, Immutable, KnownLayout)]
#[repr(C)]
struct Record {
    kind: u8,
    units: u8,
    entries: u8,
    placeholder: u8,
    modifier: u8,
    kana: u8,
    prolonged: u8,
    inline_bytes: u8,
    extra_bytes: u8,
    unprintable_bytes: u8,
    tertiary_bytes: u8,
    reserved: u8,
}

pub(super) struct Weight {
    pub kind: u8,
    pub units: usize,
    pub entries: usize,
    pub placeholder: u8,
    pub modifier: u8,
    pub kana: u8,
    pub prolonged: bool,
    pub inline: &'static [u8],
    pub extra: &'static [u8],
    pub unprintable: &'static [u8],
    pub tertiary: &'static [u8],
}
pub(super) fn lookup(family: GeneralSortFamily, unit: u16) -> Result<Weight, InspectError> {
    let table = match family {
        GeneralSortFamily::Legacy => LEGACY,
        GeneralSortFamily::General2010 => GENERAL,
    };
    if table.get(..FILE_HEADER_BYTES) != Some(b"TCGW\x01\0\0\0") {
        return Err(InspectError::Malformed);
    }
    let offset = FILE_HEADER_BYTES + usize::from(unit) * size_of::<u32>();
    let value = U32::<LittleEndian>::read_from_bytes(
        table
            .get(offset..offset + size_of::<u32>())
            .ok_or(InspectError::Malformed)?,
    )
    .map_err(|_| InspectError::Malformed)?;
    let payload = table
        .get(RECORDS_OFFSET + value.get() as usize..)
        .ok_or(InspectError::Malformed)?;
    let (record, mut payload) =
        Record::ref_from_prefix(payload).map_err(|_| InspectError::Malformed)?;
    if record.kind > REPEAT || record.reserved != 0 {
        return Err(InspectError::Malformed);
    }
    let mut take = |length: u8| {
        let (head, tail) = payload
            .split_at_checked(usize::from(length))
            .ok_or(InspectError::Malformed)?;
        payload = tail;
        Ok::<_, InspectError>(head)
    };
    Ok(Weight {
        kind: record.kind,
        units: usize::from(record.units),
        entries: usize::from(record.entries),
        placeholder: record.placeholder,
        modifier: record.modifier,
        kana: record.kana,
        prolonged: record.prolonged != 0,
        inline: take(record.inline_bytes)?,
        extra: take(record.extra_bytes)?,
        unprintable: take(record.unprintable_bytes)?,
        tertiary: take(record.tertiary_bytes)?,
    })
}
