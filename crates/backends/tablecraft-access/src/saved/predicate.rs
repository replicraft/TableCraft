use super::SavedSelectError;
use tablecraft_ace::SavedQueryRow;
use tablecraft_sql::select::SelectPredicate;

const STAR: i16 = 0x01;
const DISTINCT: i16 = 0x02;
const OWNER_ACCESS: i16 = 0x04;
const DISTINCT_ROW: i16 = 0x08;
const TOP: i16 = 0x10;
const PERCENT: i16 = 0x20;
const KNOWN_FLAGS: i16 = STAR | DISTINCT | OWNER_ACCESS | DISTINCT_ROW | TOP | PERCENT;
const PERCENT_TOTAL: u32 = 100;

pub(super) struct Predicate {
    pub syntax: SelectPredicate,
    pub star: bool,
    pub owner_access: bool,
}
pub(super) fn read(row: Option<&SavedQueryRow>) -> Result<Predicate, SavedSelectError> {
    let flag = row.and_then(SavedQueryRow::flag).unwrap_or(0);
    let count_text = row.and_then(SavedQueryRow::name1);
    if flag & !KNOWN_FLAGS != 0 {
        return Err(SavedSelectError::Unsupported);
    }
    if flag & DISTINCT != 0 && flag & DISTINCT_ROW != 0 {
        return Err(SavedSelectError::InvalidMetadata);
    }
    let top = if flag & TOP != 0 {
        let text = count_text
            .filter(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
            .ok_or(SavedSelectError::InvalidMetadata)?;
        let count = text
            .parse::<u32>()
            .map_err(|_| SavedSelectError::InvalidMetadata)?;
        let percent = flag & PERCENT != 0;
        if percent && count > PERCENT_TOTAL {
            return Err(SavedSelectError::InvalidMetadata);
        }
        Some((count, percent))
    } else {
        if count_text.is_some() || flag & PERCENT != 0 {
            return Err(SavedSelectError::InvalidMetadata);
        }
        None
    };
    let syntax = match (flag & DISTINCT != 0, flag & DISTINCT_ROW != 0, top) {
        (true, _, Some((count, percent))) => SelectPredicate::DistinctTop { count, percent },
        (_, true, Some((count, percent))) => SelectPredicate::DistinctRowTop { count, percent },
        (_, _, Some((count, percent))) => SelectPredicate::Top { count, percent },
        (true, _, None) => SelectPredicate::Distinct,
        (_, true, None) => SelectPredicate::DistinctRow,
        _ => SelectPredicate::All,
    };
    Ok(Predicate {
        syntax,
        star: flag & STAR != 0,
        owner_access: flag & OWNER_ACCESS != 0,
    })
}
