use super::{Error, declaration, line};
use crate::{native, output::TableOutput};

pub(super) fn run<R: tablecraft_ace::NativeReadSource>(
    source: &mut R,
    output: &mut TableOutput,
    mut budget: tablecraft_core::limits::ResourceBudget,
) -> Result<(), Error> {
    let relationships =
        tablecraft_ace::inspect_relationships(source, native::CATALOG_SCOPE, &mut budget, &|| {
            false
        })
        .map_err(Error::Read)?;
    output.table_section().map_err(Error::Output)?;
    line(
        output,
        &[
            "Relationship",
            "Parent table",
            "Child table",
            "Flags",
            "Enforced",
            "One-to-one",
            "Update",
            "Delete",
            "Ordinal",
            "Parent column",
            "Child column",
        ],
    )?;
    for native in relationships {
        let relationship = native.declaration();
        let rules = relationship.rules();
        let update = rules
            .map(|rules| format!("{:?}", rules.on_update))
            .unwrap_or_else(|| "UNKNOWN".into());
        let delete = rules
            .map(|rules| format!("{:?}", rules.on_delete))
            .unwrap_or_else(|| "UNKNOWN".into());
        for (ordinal, column) in relationship.columns().iter().enumerate() {
            line(
                output,
                &[
                    relationship.name().as_str(),
                    relationship.parent_table().as_str(),
                    relationship.child_table().as_str(),
                    &format!("0x{:08x}", native.flags()),
                    declaration(rules.map(|rules| rules.enforced)),
                    declaration(rules.map(|rules| rules.one_to_one)),
                    &update,
                    &delete,
                    &ordinal.to_string(),
                    column.parent().as_str(),
                    column.child().as_str(),
                ],
            )?;
        }
    }
    Ok(())
}
