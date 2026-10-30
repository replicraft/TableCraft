//! Explicit observations of all retained Extra properties; never event discovery authority.
use super::{DataMacroXml, TableExtra};
use crate::{InspectError, header::check};
use tablecraft_core::limits::{Resource, ResourceBudget};

/// XML observations for one parsed original property. These variants do not
/// classify native macro storage, required coverage or executable semantics.
pub enum MacroPropertyObservation {
    /// Qualified XML envelopes; event/name attributes remain observed strings.
    Envelopes(DataMacroXml),
    /// The original property has no text in the supported carrier profile.
    NoText,
    /// Text exists, but its XML namespace/root is outside the envelope reader.
    UnsupportedXml,
    /// Qualified text was not well-formed under the existing safe XML reader.
    MalformedXml,
}

/// One parsed source-property observation in original group/property order.
pub struct MacroPropertyInventory {
    group: usize,
    property: usize,
    observation: MacroPropertyObservation,
}

impl MacroPropertyInventory {
    /// Index into the retained source's original groups.
    pub const fn group_ordinal(&self) -> usize {
        self.group
    }

    /// Index into that group's original parsed properties.
    pub const fn property_ordinal(&self) -> usize {
        self.property
    }

    /// Observed envelope facts or a specific unresolved property profile.
    pub fn observation(&self) -> &MacroPropertyObservation {
        &self.observation
    }
}

/// A complete bounded traversal of the already-retained Extra reader result.
/// Original object/native-parent/catalog-root identity and exact payload/group/
/// property bytes remain available through `source`. This is not a native macro
/// locator, absence certificate, execution capability or save permission.
pub struct TableMacroInventory<'a> {
    source: &'a TableExtra,
    properties: Vec<MacroPropertyInventory>,
    opaque_groups: Vec<usize>,
    unrepresented_names: Vec<usize>,
}

impl TableMacroInventory<'_> {
    /// Original table-owned carrier, including identities, bytes and coverage.
    pub fn source(&self) -> &TableExtra {
        self.source
    }

    /// Every parsed property, including unsupported/nontext/malformed values.
    pub fn properties(&self) -> &[MacroPropertyInventory] {
        &self.properties
    }

    /// Source group ordinals whose bodies remain opaque.
    pub fn opaque_groups(&self) -> &[usize] {
        &self.opaque_groups
    }

    /// Source name-list ordinals referenced by no parsed property. The retained
    /// reader enforces a unique name bank and clones the exact indexed name into
    /// each parsed property, so exact equality recovers that association.
    /// Opaque groups may contain unknown references to these same names; this is
    /// not evidence that a name is globally unused or a macro is absent.
    pub fn unrepresented_names(&self) -> &[usize] {
        &self.unrepresented_names
    }
}

impl TableExtra {
    /// Explicitly inspect all parsed properties with the existing inert XML
    /// envelope reader. Unknown values remain observations. Allocation, resource,
    /// structural-limit and cancellation failures return no partial inventory.
    /// Unselected/unknown bodies are never evaluated or turned into selectors.
    pub fn inspect_macro_inventory(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<TableMacroInventory<'_>, InspectError> {
        let _span = tracing::debug_span!("native.table.extra.macro_inventory.inspect").entered();
        check(budget, cancelled)?;
        let mut property_count = 0_usize;
        let mut opaque_count = 0_usize;
        for group in self.groups() {
            check(budget, cancelled)?;
            if let Some(properties) = group.properties() {
                property_count = property_count
                    .checked_add(properties.len())
                    .ok_or(InspectError::StructureLimit)?;
            } else {
                opaque_count = opaque_count
                    .checked_add(1)
                    .ok_or(InspectError::StructureLimit)?;
            }
        }
        let mut properties = vector(property_count, budget)?;
        let mut opaque_groups = vector(opaque_count, budget)?;
        let mut unrepresented_names = vector(self.names().len(), budget)?;

        for (group_ordinal, group) in self.groups().iter().enumerate() {
            check(budget, cancelled)?;
            let Some(parsed) = group.properties() else {
                opaque_groups.push(group_ordinal);
                continue;
            };
            for (property_ordinal, property) in parsed.iter().enumerate() {
                check(budget, cancelled)?;
                let observation = if property.text().is_none() {
                    MacroPropertyObservation::NoText
                } else {
                    match property.inspect_macro_xml(budget, cancelled) {
                        Ok(xml) => MacroPropertyObservation::Envelopes(xml),
                        Err(InspectError::Unsupported) => MacroPropertyObservation::UnsupportedXml,
                        Err(InspectError::Malformed) => MacroPropertyObservation::MalformedXml,
                        Err(error) => return Err(error),
                    }
                };
                properties.push(MacroPropertyInventory {
                    group: group_ordinal,
                    property: property_ordinal,
                    observation,
                });
            }
        }

        for (name_ordinal, name) in self.names().iter().enumerate() {
            let mut represented = false;
            for group in self.groups() {
                check(budget, cancelled)?;
                if let Some(parsed) = group.properties() {
                    for property in parsed {
                        check(budget, cancelled)?;
                        let work = name
                            .len()
                            .checked_add(property.name().len())
                            .ok_or(InspectError::StructureLimit)?;
                        budget.charge(Resource::Steps, work as u64)?;
                        if name == property.name() {
                            represented = true;
                        }
                    }
                }
            }
            if !represented {
                unrepresented_names.push(name_ordinal);
            }
        }
        check(budget, cancelled)?;

        Ok(TableMacroInventory {
            source: self,
            properties,
            opaque_groups,
            unrepresented_names,
        })
    }
}

fn vector<T>(count: usize, budget: &mut ResourceBudget) -> Result<Vec<T>, InspectError> {
    budget.charge_allocation(count as u64, size_of::<T>() as u64)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;
    Ok(result)
}
