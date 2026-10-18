use super::*;
use std::fmt::{self, Display, Formatter};
pub(crate) fn names(f: &mut Formatter<'_>, fields: &[Name]) -> fmt::Result {
    f.write_str("(")?;
    for (i, field) in fields.iter().enumerate() {
        if i > 0 {
            f.write_str(", ")?;
        }
        field.fmt(f)?;
    }
    f.write_str(")")
}
impl Display for Definition {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::CreateTable(table) => {
                write!(
                    f,
                    "CREATE {}TABLE {} (",
                    if table.temporary { "TEMPORARY " } else { "" },
                    table.name
                )?;
                for (i, element) in table.elements.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    match element {
                        TableElement::Column(column) => column.fmt(f)?,
                        TableElement::Constraint(constraint) => constraint.fmt(f)?,
                    }
                }
                f.write_str(")")
            }
            Self::AlterTable(table) => {
                write!(f, "ALTER TABLE {} ", table.name)?;
                match &table.operation {
                    AlterOperation::AddColumn(column) => write!(f, "ADD COLUMN {column}"),
                    AlterOperation::AlterColumn { name, data_type } => {
                        write!(f, "ALTER COLUMN {name} {data_type}")
                    }
                    AlterOperation::AddConstraint(constraint) => write!(f, "ADD {constraint}"),
                    AlterOperation::DropColumn(name) => write!(f, "DROP COLUMN {name}"),
                    AlterOperation::DropConstraint(name) => write!(f, "DROP CONSTRAINT {name}"),
                }
            }
            Self::CreateIndex(index) => {
                write!(
                    f,
                    "CREATE {}INDEX {} ON {} (",
                    if index.unique { "UNIQUE " } else { "" },
                    index.name,
                    index.table
                )?;
                for (i, field) in index.fields.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    write!(
                        f,
                        "{} {}",
                        field.name,
                        if field.descending { "DESC" } else { "ASC" }
                    )?;
                }
                f.write_str(")")?;
                if let Some(policy) = index.policy {
                    f.write_str(match policy {
                        IndexPolicy::Primary => " WITH PRIMARY",
                        IndexPolicy::DisallowNull => " WITH DISALLOW NULL",
                        IndexPolicy::IgnoreNull => " WITH IGNORE NULL",
                    })?;
                }
                Ok(())
            }
            Self::DropTable(name) => write!(f, "DROP TABLE {name}"),
            Self::DropIndex { index, table } => write!(f, "DROP INDEX {index} ON {table}"),
        }
    }
}
impl Display for Column {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.name, self.data_type)?;
        if self.not_null {
            f.write_str(" NOT NULL")?;
        }
        if self.compression {
            f.write_str(" WITH COMPRESSION")?;
        }
        if let Some(constraint) = &self.constraint {
            write!(f, " {constraint}")?;
        }
        Ok(())
    }
}
impl Display for Constraint {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if let Some(name) = &self.name {
            write!(f, "CONSTRAINT {name} ")?;
        }
        let fields = match &self.kind {
            ConstraintKind::PrimaryKey(fields) => {
                f.write_str("PRIMARY KEY")?;
                fields
            }
            ConstraintKind::Unique(fields) => {
                f.write_str("UNIQUE")?;
                fields
            }
            ConstraintKind::NotNull(fields) => {
                f.write_str("NOT NULL")?;
                fields
            }
            ConstraintKind::ForeignKey(reference) => {
                if !reference.fields.is_empty() {
                    f.write_str("FOREIGN KEY ")?;
                    if reference.no_index {
                        f.write_str("NO INDEX ")?;
                    }
                    names(f, &reference.fields)?;
                    f.write_str(" ")?;
                }
                write!(f, "REFERENCES {}", reference.table)?;
                if !reference.foreign_fields.is_empty() {
                    f.write_str(" ")?;
                    names(f, &reference.foreign_fields)?;
                }
                for (event, action) in [
                    ("UPDATE", reference.on_update),
                    ("DELETE", reference.on_delete),
                ] {
                    if let Some(action) = action {
                        write!(
                            f,
                            " ON {event} {}",
                            match action {
                                ReferentialAction::Cascade => "CASCADE",
                                ReferentialAction::SetNull => "SET NULL",
                            }
                        )?;
                    }
                }
                return Ok(());
            }
        };
        if !fields.is_empty() {
            f.write_str(" ")?;
            names(f, fields)?;
        }
        Ok(())
    }
}
