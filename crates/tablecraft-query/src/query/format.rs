use super::*;
use crate::select::format::sort;
use std::fmt::{self, Display, Formatter};
impl Display for Query {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.fmt_sql(f)?;
        f.write_str(";")
    }
}
impl Query {
    pub(crate) fn fmt_sql(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if !self.parameters.is_empty() {
            f.write_str("PARAMETERS ")?;
            parameters(f, &self.parameters)?;
            f.write_str("; ")?;
        }
        match &self.statement {
            Statement::Definition(definition) => definition.fmt(f)?,
            Statement::Read(read) => read.fmt_sql(f)?,
            Statement::MakeTable(select) => {
                select.fmt_body(f)?;
                sort(f, &select.order_by)?;
                if select.owner_access {
                    f.write_str(" WITH OWNERACCESS OPTION")?;
                }
            }
            Statement::Crosstab(cross) => {
                write!(f, "TRANSFORM {} ", cross.transform)?;
                cross.select.fmt_body(f)?;
                sort(f, &cross.select.order_by)?;
                write!(f, " PIVOT {}", cross.pivot)?;
                if let Some(headings) = &cross.headings {
                    f.write_str(" IN (")?;
                    expr_list(f, headings)?;
                    f.write_str(")")?;
                }
            }
            Statement::Insert(insert) => {
                write!(f, "INSERT INTO {}", insert.target)?;
                if !insert.fields.is_empty() {
                    f.write_str(" (")?;
                    for (i, field) in insert.fields.iter().enumerate() {
                        if i > 0 {
                            f.write_str(", ")?;
                        }
                        field.fmt(f)?;
                    }
                    f.write_str(")")?;
                }
                if let Some(external) = &insert.external {
                    external.fmt(f)?;
                }
                match &insert.source {
                    InsertSource::Values(values) => {
                        f.write_str(" VALUES (")?;
                        expr_list(f, values)?;
                        f.write_str(")")?;
                    }
                    InsertSource::Query(query) => {
                        f.write_str(" ")?;
                        query.fmt_sql(f)?;
                    }
                }
            }
            Statement::Update(update) => {
                f.write_str("UPDATE ")?;
                sources(f, &update.sources)?;
                f.write_str(" SET ")?;
                for (i, assign) in update.assignments.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{} = {}", assign.target, assign.value)?;
                }
                if let Some(selection) = &update.selection {
                    write!(f, " WHERE {selection}")?;
                }
            }
            Statement::Delete(delete) => {
                f.write_str("DELETE ")?;
                for (i, target) in delete.targets.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{target}.*")?;
                }
                if !delete.targets.is_empty() {
                    f.write_str(" ")?;
                }
                f.write_str("FROM ")?;
                sources(f, &delete.sources)?;
                if let Some(selection) = &delete.selection {
                    write!(f, " WHERE {selection}")?;
                }
            }
        }
        Ok(())
    }
}
pub(crate) fn parameters(f: &mut Formatter<'_>, parameters: &[Parameter]) -> fmt::Result {
    for (i, parameter) in parameters.iter().enumerate() {
        if i > 0 {
            f.write_str(", ")?;
        }
        write!(f, "{} {}", parameter.name, parameter.data_type)?;
    }
    Ok(())
}
pub(crate) fn expr_list(
    f: &mut Formatter<'_>,
    values: &[crate::select::Expression],
) -> fmt::Result {
    for (i, value) in values.iter().enumerate() {
        if i > 0 {
            f.write_str(", ")?;
        }
        value.fmt(f)?;
    }
    Ok(())
}
fn sources(f: &mut Formatter<'_>, sources: &[crate::select::TableSource]) -> fmt::Result {
    for (i, source) in sources.iter().enumerate() {
        if i > 0 {
            f.write_str(", ")?;
        }
        source.fmt(f)?;
    }
    Ok(())
}
impl ReadBody {
    pub(crate) fn fmt_sql(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Select(select) => select.fmt_body(f),
            Self::Reference(name) => name.fmt(f),
            Self::Table(name) => write!(f, "TABLE {name}"),
            Self::Union { first, rest } => {
                first.fmt_sql(f)?;
                for arm in rest {
                    f.write_str(if arm.all { " UNION ALL " } else { " UNION " })?;
                    arm.body.fmt_sql(f)?;
                }
                Ok(())
            }
        }
    }
}
impl ReadQuery {
    pub(crate) fn fmt_sql(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.body.fmt_sql(f)?;
        sort(f, &self.order_by)?;
        if self.owner_access {
            f.write_str(" WITH OWNERACCESS OPTION")?;
        }
        Ok(())
    }
}
