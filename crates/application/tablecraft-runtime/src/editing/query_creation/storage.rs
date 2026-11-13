//! Native-reviewed technical storage facts, separate from user schema receipts.
#[cfg(any(target_os = "macos", target_os = "linux", windows))]
use super::QueryCreatedReopen;

/// Technical catalog change selected by the actual native QueryDef plan.
/// Constructing this descriptive value never constructs a reopen witness.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueryCreationStorageChange {
    /// The operation reused the existing MSysQueries storage.
    Existing,
    /// The operation initialized exactly the missing local system role.
    Initialized {
        native_table_id: i32,
        definition_page: u32,
    },
}

pub(in crate::editing) struct Witness {
    change: QueryCreationStorageChange,
    storage_display: Option<String>,
    #[cfg(any(target_os = "macos", target_os = "linux", windows))]
    prior: Vec<TableFact>,
    #[cfg(any(target_os = "macos", target_os = "linux", windows))]
    initialized: Option<TableFact>,
}

impl Witness {
    pub(in crate::editing) fn change(&self) -> QueryCreationStorageChange {
        self.change
    }

    pub(in crate::editing) fn display(&self) -> Option<&str> {
        self.storage_display.as_deref()
    }
}

#[cfg(any(target_os = "macos", target_os = "linux", windows))]
struct TableFact {
    name: String,
    native_id: i32,
    parent: i32,
    kind: i16,
    flags: u32,
}

#[cfg(any(target_os = "macos", target_os = "linux", windows))]
mod native {
    use super::{QueryCreatedReopen, QueryCreationStorageChange, TableFact, Witness};
    use crate::{
        Error,
        controls::{SCOPE, check},
        display::Presentation,
    };
    use std::io::Cursor;
    use tablecraft_ace::{
        CatalogInspection, NativeImage, NativeImageSource, NativeObjectOrigin,
        NativeQueryStorageChange, NativeTableStorage, RecoverableNativeQueryCreationPlan,
        inspect_catalog, inspect_table_schema,
    };
    use tablecraft_core::limits::{Resource, ResourceBudget};
    use tablecraft_model::{ObjectDescriptor, ObjectKind};

    const QUERY_STORAGE: &str = "MSysQueries";
    const SYSTEM_FLAGS: u32 = 0x8000_0002;

    fn entries(
        catalog: &CatalogInspection,
    ) -> impl Iterator<Item = (&ObjectDescriptor, &NativeObjectOrigin)> {
        catalog.catalog().objects().iter().zip(catalog.origins())
    }

    impl TableFact {
        fn capture(
            object: &ObjectDescriptor,
            origin: &NativeObjectOrigin,
            output: &mut Presentation<'_>,
        ) -> Result<Self, Error> {
            Ok(Self {
                // NO_CLONE: Guarded reopen outlives this temporary catalog. Keep
                // only its bounded name, rather than retaining another whole image.
                name: output.text(object.name().as_str())?,
                native_id: origin.native_id(),
                parent: origin.parent_id(),
                kind: origin.type_code(),
                flags: origin.flags(),
            })
        }

        fn matches(
            &self,
            object: &ObjectDescriptor,
            origin: &NativeObjectOrigin,
            work: &mut ResourceBudget,
            cancelled: &dyn Fn() -> bool,
        ) -> Result<bool, Error> {
            check(cancelled)?;
            work.charge(
                Resource::Steps,
                self.name.len().max(object.name().as_str().len()) as u64,
            )?;
            Ok(self.name == object.name().as_str()
                && self.native_id == origin.native_id()
                && self.parent == origin.parent_id()
                && self.kind == origin.type_code()
                && self.flags == origin.flags())
        }
    }

    impl Witness {
        pub(in crate::editing) fn capture(
            source: &NativeImageSource,
            plan: &RecoverableNativeQueryCreationPlan,
            work: &mut ResourceBudget,
            output: &mut Presentation<'_>,
            cancelled: &dyn Fn() -> bool,
        ) -> Result<Self, Error> {
            let (native_table_id, definition_page) = match plan.storage_change() {
                NativeQueryStorageChange::Existing => {
                    return Ok(Self {
                        change: QueryCreationStorageChange::Existing,
                        storage_display: None,
                        prior: Vec::new(),
                        initialized: None,
                    });
                }
                NativeQueryStorageChange::Initialized {
                    native_table_id,
                    definition_page,
                } => (native_table_id, definition_page),
            };

            check(cancelled)?;
            let before =
                inspect_catalog(&mut Cursor::new(source.as_bytes()), SCOPE, work, cancelled)?;
            let after =
                inspect_catalog(&mut Cursor::new(plan.after_bytes()), SCOPE, work, cancelled)?;

            let mut count = 0_usize;
            for (object, _) in entries(&before) {
                check(cancelled)?;
                work.charge(Resource::Steps, 1)?;
                if object.kind() == ObjectKind::Table {
                    count = count
                        .checked_add(1)
                        .ok_or(Error::QueryCreatedSourceChanged)?;
                }
            }

            work.charge_allocation(count as u64, size_of::<TableFact>() as u64)?;
            let mut prior = Vec::new();
            prior.try_reserve_exact(count)?;
            for (object, origin) in entries(&before) {
                check(cancelled)?;
                work.charge(Resource::Steps, 1)?;
                if object.kind() != ObjectKind::Table {
                    continue;
                }
                work.charge(Resource::Steps, object.name().as_str().len() as u64)?;
                if object.name().as_str() == QUERY_STORAGE || origin.native_id() == native_table_id
                {
                    return Err(Error::QueryCreatedSourceChanged);
                }
                prior.push(TableFact::capture(object, origin, output)?);
            }

            work.charge(Resource::Steps, after.origins().len() as u64)?;
            let mut parents = after
                .catalog()
                .objects()
                .iter()
                .zip(after.origins())
                .filter(|(object, origin)| {
                    object.name().as_str() == "Tables"
                        && origin.type_code() == 3
                        && origin.is_system()
                });
            let (_, parent) = parents.next().ok_or(Error::QueryCreatedSourceChanged)?;
            if parents.next().is_some() {
                return Err(Error::QueryCreatedSourceChanged);
            }

            let mut selected = None;
            for (object, origin) in entries(&after) {
                check(cancelled)?;
                work.charge(Resource::Steps, 1)?;
                if object.kind() != ObjectKind::Table {
                    continue;
                }
                work.charge(Resource::Steps, object.name().as_str().len() as u64)?;
                if object.name().as_str() == QUERY_STORAGE || origin.native_id() == native_table_id
                {
                    if selected.is_some()
                        || object.name().as_str() != QUERY_STORAGE
                        || origin.native_id() != native_table_id
                        || origin.type_code() != 1
                        || origin.flags() != SYSTEM_FLAGS
                        || origin.parent_id() != parent.native_id()
                        || origin.table_storage() != Some(NativeTableStorage::Local)
                    {
                        return Err(Error::QueryCreatedSourceChanged);
                    }
                    selected = Some((object, origin));
                }
            }
            let (object, origin) = selected.ok_or(Error::QueryCreatedSourceChanged)?;
            let schema = inspect_table_schema(
                &mut Cursor::new(plan.after_bytes()),
                SCOPE,
                QUERY_STORAGE,
                work,
                cancelled,
            )?;
            if schema.definition_page() != definition_page {
                return Err(Error::QueryCreatedSourceChanged);
            }

            let result = Self {
                change: QueryCreationStorageChange::Initialized {
                    native_table_id,
                    definition_page,
                },
                storage_display: Some(output.text(&format!("{:?}", origin.table_storage()))?),
                prior,
                initialized: Some(TableFact::capture(object, origin, output)?),
            };

            result.verify_catalog(&after, work, cancelled)?;
            Ok(result)
        }

        fn verify_catalog(
            &self,
            catalog: &CatalogInspection,
            work: &mut ResourceBudget,
            cancelled: &dyn Fn() -> bool,
        ) -> Result<(), Error> {
            let native_table_id = match self.change {
                QueryCreationStorageChange::Existing => return Ok(()),
                QueryCreationStorageChange::Initialized {
                    native_table_id, ..
                } => native_table_id,
            };

            let expected = self
                .initialized
                .as_ref()
                .ok_or(Error::QueryCreatedSourceChanged)?;
            let mut prior = self.prior.iter();
            let mut found = false;
            for (object, origin) in entries(catalog) {
                check(cancelled)?;
                work.charge(Resource::Steps, 1)?;
                if object.kind() != ObjectKind::Table {
                    continue;
                }
                if origin.native_id() == native_table_id {
                    if found || !expected.matches(object, origin, work, cancelled)? {
                        return Err(Error::QueryCreatedSourceChanged);
                    }
                    found = true;
                } else if !prior
                    .next()
                    .ok_or(Error::QueryCreatedSourceChanged)?
                    .matches(object, origin, work, cancelled)?
                {
                    return Err(Error::QueryCreatedSourceChanged);
                }
            }

            if !found || prior.next().is_some() {
                return Err(Error::QueryCreatedSourceChanged);
            }
            Ok(())
        }
    }

    impl QueryCreatedReopen {
        pub(crate) fn verify_storage(
            &self,
            image: &NativeImage,
            work: &mut ResourceBudget,
            cancelled: &dyn Fn() -> bool,
        ) -> Result<(), Error> {
            self.storage
                .verify_catalog(image.catalog(), work, cancelled)?;
            if let QueryCreationStorageChange::Initialized {
                definition_page, ..
            } = self.storage.change
            {
                let schema = inspect_table_schema(
                    &mut image.reader(),
                    SCOPE,
                    QUERY_STORAGE,
                    work,
                    cancelled,
                )?;
                if schema.definition_page() != definition_page {
                    return Err(Error::QueryCreatedSourceChanged);
                }
            }
            check(cancelled)
        }
    }
}
