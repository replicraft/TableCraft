//! Profile-semantic NULL, uniqueness and distinct-family checks.
use crate::{
    EditError, IndexConstraintViolation, InspectError, NativeIndex,
    format::reader::Reader,
    table::indexes::{key::KeyProfile, page::KeyRef},
};
use std::io::Cursor;

pub(in crate::table::indexes) fn require_population<P: KeyProfile>(
    index: &NativeIndex,
    keys: &[KeyRef<P::Key>],
    profile: &P,
    live_rows: usize,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    let constraints = index.constraints().ok_or(InspectError::Unsupported)?;
    if constraints.ignore_nulls() {
        if keys.len() > live_rows {
            return Err(EditError::Preservation);
        }
        for entry in keys {
            reader.tick()?;
            if profile.constraint_nulls(&entry.key)?.0 {
                return Err(EditError::Preservation);
            }
        }
    } else if keys.len() != live_rows {
        return Err(EditError::Preservation);
    }
    Ok(())
}

pub(in crate::table::indexes) fn require_constraints<P: KeyProfile>(
    index: &NativeIndex,
    keys: &[KeyRef<P::Key>],
    profile: &P,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    let constraints = index.constraints().ok_or(InspectError::Unsupported)?;
    let mut previous = None;
    for entry in keys {
        reader.tick()?;
        let (all_null, any_null) = profile.constraint_nulls(&entry.key)?;
        if constraints.ignore_nulls() && all_null {
            return Err(EditError::Preservation);
        }
        if constraints.required() && any_null {
            return Err(InspectError::IndexConstraint(IndexConstraintViolation::NullKey).into());
        }
        if constraints.unique() && !profile.unique_key_is_exact(&entry.key) {
            return Err(InspectError::Unsupported.into());
        }
        if !all_null {
            if constraints.unique() {
                reader.work(profile.max_key_bytes())?;
                if previous.is_some_and(|prior| profile.compare(prior, &entry.key).is_eq()) {
                    return Err(InspectError::IndexConstraint(
                        IndexConstraintViolation::DuplicateKey,
                    )
                    .into());
                }
            }
            previous = Some(&entry.key);
        }
    }
    Ok(())
}

pub(super) fn distinct<P: KeyProfile>(
    keys: &[KeyRef<P::Key>],
    profile: &P,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<usize, EditError> {
    let mut count = 0;
    for (position, entry) in keys.iter().enumerate() {
        reader.work(profile.max_key_bytes())?;
        if position == 0 || !profile.compare(&keys[position - 1].key, &entry.key).is_eq() {
            count += 1;
        }
    }
    Ok(count)
}
