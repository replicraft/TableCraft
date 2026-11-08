//! Inert explicit-OFF observations; missing settings never select a default.
use super::{DatabaseProperties, PropertyGroupKind};
use crate::{InspectError, format::data_type, header::check};
use tablecraft_core::limits::{Resource, ResourceBudget};

const TRACK_NAMES: &str = "Track Name AutoCorrect Info";
const PERFORM_NAMES: &str = "Perform Name AutoCorrect";
const DDL_FLAG: u8 = 1;

/// Observed unnamed-map settings, independently of file or UI release.
/// A missing MSysDb role is separately represented by the inspection's None.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DatabaseNameAutoCorrect {
    /// Neither named property occurs in an unnamed default map.
    Absent,
    /// Both settings occur once as native LONG/DDL zero.
    ExplicitOff,
    /// Partial, duplicate, differently typed/flagged or other stored values.
    /// This does not assert an enabled representation or NameMap behavior.
    Other,
}

impl DatabaseProperties {
    /// Observe only the two named settings in unnamed default maps.
    /// This inert fact neither admits a mutation nor supplies missing defaults.
    pub fn name_auto_correct(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<DatabaseNameAutoCorrect, InspectError> {
        let mut track = false;
        let mut perform = false;
        let mut qualified = true;
        for group in self.groups() {
            check(budget, cancelled)?;
            if group.kind() != PropertyGroupKind::Table || !group.name().is_empty() {
                continue;
            }
            for property in group.properties() {
                check(budget, cancelled)?;
                budget.charge(
                    Resource::Steps,
                    (2 * property.name().len() + TRACK_NAMES.len() + PERFORM_NAMES.len()) as u64,
                )?;
                let seen = if property.name().eq_ignore_ascii_case(TRACK_NAMES) {
                    &mut track
                } else if property.name().eq_ignore_ascii_case(PERFORM_NAMES) {
                    &mut perform
                } else {
                    continue;
                };
                budget.charge(Resource::Steps, property.raw_value().len() as u64)?;
                qualified &= !*seen
                    && property.native_type() == data_type::INT32
                    && property.flags() == DDL_FLAG
                    && property.raw_value() == 0_i32.to_le_bytes();
                *seen = true;
            }
        }
        check(budget, cancelled)?;
        Ok(if !track && !perform {
            DatabaseNameAutoCorrect::Absent
        } else if track && perform && qualified {
            DatabaseNameAutoCorrect::ExplicitOff
        } else {
            DatabaseNameAutoCorrect::Other
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inspect_database_properties;
    use std::io::Cursor;
    use tablecraft_model::CatalogScope;
    use tablecraft_testkit::fixtures::access::{
        canonical_creator_rename, canonical_database_options, database_properties,
    };

    fn budget() -> ResourceBudget {
        ResourceBudget::new(64 * 1024 * 1024, 100_000, 10_000_000)
    }

    fn inspect(bytes: &[u8]) -> Option<DatabaseProperties> {
        inspect_database_properties(
            &mut Cursor::new(bytes),
            CatalogScope::new(749),
            &mut budget(),
            &|| false,
        )
        .unwrap()
    }

    fn rename_property(bytes: &mut [u8], name: &str) {
        let raw = name
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        let at = bytes
            .windows(raw.len())
            .position(|bytes| bytes == raw)
            .unwrap();
        bytes[at] = b'X';
    }

    #[test]
    fn absence_and_explicit_off_are_distinct_without_invented_enabled_defaults() {
        let original = canonical_creator_rename::originals().remove(0);
        assert!(inspect(&original).is_none());
        let no_values = database_properties::database(None);
        assert_eq!(
            inspect(&no_values)
                .unwrap()
                .name_auto_correct(&mut budget(), &|| false)
                .unwrap(),
            DatabaseNameAutoCorrect::Absent
        );
        let explicit = canonical_database_options::explicit_off(&original);
        assert_eq!(
            inspect(&explicit)
                .unwrap()
                .name_auto_correct(&mut budget(), &|| false)
                .unwrap(),
            DatabaseNameAutoCorrect::ExplicitOff
        );
        let mut absent = explicit.clone();
        rename_property(&mut absent, TRACK_NAMES);
        rename_property(&mut absent, PERFORM_NAMES);
        assert_eq!(
            inspect(&absent)
                .unwrap()
                .name_auto_correct(&mut budget(), &|| false)
                .unwrap(),
            DatabaseNameAutoCorrect::Absent
        );
        let mut partial = explicit.clone();
        rename_property(&mut partial, TRACK_NAMES);
        let mut value = explicit.clone();
        *value.last_mut().unwrap() = 1;
        let mut kind = explicit.clone();
        let at = kind.len() - 9;
        assert_eq!(kind[at], 4);
        kind[at] = 0x7f;
        let mut flags = explicit;
        let at = flags.len() - 10;
        assert_eq!(flags[at], 1);
        flags[at] = 0;
        for bytes in [partial, value, flags, kind] {
            assert_eq!(
                inspect(&bytes)
                    .unwrap()
                    .name_auto_correct(&mut budget(), &|| false)
                    .unwrap(),
                DatabaseNameAutoCorrect::Other
            );
        }
    }

    #[test]
    fn option_observation_respects_cancellation_and_resource_limits() {
        let explicit = canonical_database_options::explicit_off(
            &canonical_creator_rename::originals().remove(0),
        );
        let properties = inspect(&explicit).unwrap();
        assert!(matches!(
            properties.name_auto_correct(&mut budget(), &|| true),
            Err(InspectError::Cancelled)
        ));
        assert!(
            properties
                .name_auto_correct(&mut ResourceBudget::new(0, 0, 0), &|| false)
                .is_err()
        );
    }
}
