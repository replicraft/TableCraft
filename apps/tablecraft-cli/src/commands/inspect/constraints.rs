use super::{Error, line};
use crate::{native, output::TableOutput};
use tablecraft_ace::{RelationshipCheckStatus, RuleCheckStatus, check_table_constraints};

pub(super) fn run<R: tablecraft_ace::NativeReadSource>(
    source: &mut R,
    name: &str,
    output: &mut TableOutput,
    mut budget: tablecraft_core::limits::ResourceBudget,
) -> Result<(), Error> {
    let report =
        check_table_constraints(source, native::CATALOG_SCOPE, name, &mut budget, &|| false)
            .map_err(Error::Read)?;
    line(output, &["Table", report.name()])?;
    line(output, &["Rows checked", &report.rows().to_string()])?;
    line(
        output,
        &["Table validation rule", status(report.table_validation())],
    )?;
    output.table_section().map_err(Error::Output)?;
    line(
        output,
        &["Field", "Required", "Allow zero length", "Validation rule"],
    )?;
    for field in report.fields() {
        line(
            output,
            &[
                field.name(),
                status(field.required()),
                status(field.empty_text()),
                status(field.validation()),
            ],
        )?;
    }
    output.table_section().map_err(Error::Output)?;
    line(output, &["Relationship", "Integrity"])?;
    for relationship in report.relationships() {
        let status = match relationship.status() {
            RelationshipCheckStatus::Checked => "Checked: exact key membership",
            RelationshipCheckStatus::NotEnforced => "Not enforced",
            RelationshipCheckStatus::Unsupported => "Not checked: unsupported profile",
        };
        line(
            output,
            &[
                relationship.relationship().declaration().name().as_str(),
                status,
            ],
        )?;
    }
    Ok(())
}
fn status(status: RuleCheckStatus) -> &'static str {
    match status {
        RuleCheckStatus::Checked => "Checked",
        RuleCheckStatus::NotDeclared => "Not declared",
        RuleCheckStatus::Unsupported => "Not checked: unsupported rule",
    }
}
