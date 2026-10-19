use super::{Error, TableOutput, line};
use crate::native;
use std::fs::File;
use tablecraft_ace::{IndexCheckStatus, check_table};

pub(super) fn run(source: &mut File, table: &str, output: &mut TableOutput) -> Result<(), Error> {
    let report = check_table(
        source,
        native::CATALOG_SCOPE,
        table,
        &mut native::budget(),
        &|| false,
    )
    .map_err(Error::Read)?;
    let checked = report
        .indexes()
        .iter()
        .filter(|index| index.status() == IndexCheckStatus::Checked)
        .count();
    line(output, &["Table", report.schema().name()])?;
    line(output, &["Rows checked", &report.rows().to_string()])?;
    line(output, &["Indexes checked", &checked.to_string()])?;
    line(
        output,
        &[
            "Indexes not checked",
            &(report.indexes().len() - checked).to_string(),
        ],
    )?;
    line(output, &["Index", "Status", "Index constraints"])?;
    for index in report.indexes() {
        let status = match index.status() {
            IndexCheckStatus::Checked => "Checked",
            IndexCheckStatus::Unsupported => "Not checked: unsupported profile",
        };
        let constraints = match index.status() {
            IndexCheckStatus::Checked => "Checked: exact unique/required",
            IndexCheckStatus::Unsupported => "Not checked",
        };
        line(output, &[index.index().name(), status, constraints])?;
    }
    Ok(())
}
