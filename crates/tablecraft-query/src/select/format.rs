use std::fmt::{self, Display, Formatter};

use super::{
    BinaryOperator, Expression, FieldReference, Name, Projection, Select, SelectPredicate,
};

impl Display for Name {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "[{}]", self.as_str())
    }
}

impl Display for FieldReference {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if let Some(qualifier) = &self.qualifier {
            write!(f, "{qualifier}.")?;
        }
        self.name.fmt(f)
    }
}

impl Display for BinaryOperator {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Equal => "=",
            Self::NotEqual => "<>",
            Self::Less => "<",
            Self::LessOrEqual => "<=",
            Self::Greater => ">",
            Self::GreaterOrEqual => ">=",
            Self::And => "AND",
            Self::Or => "OR",
        })
    }
}

impl Display for Expression {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Field(field) => field.fmt(f),
            Self::Integer(value) => value.fmt(f),
            Self::Text(value) => write!(f, "'{}'", value.replace('\'', "''")),
            Self::Null => f.write_str("NULL"),
            Self::Boolean(value) => f.write_str(if *value { "TRUE" } else { "FALSE" }),
            Self::Binary {
                left,
                operator,
                right,
            } => write!(f, "({left} {operator} {right})"),
            Self::IsNull {
                expression,
                negated,
            } => {
                write!(
                    f,
                    "({expression} IS {}NULL)",
                    if *negated { "NOT " } else { "" }
                )
            }
            Self::Between {
                expression,
                first_bound,
                second_bound,
                negated,
            } => {
                write!(
                    f,
                    "({expression} {}BETWEEN {first_bound} AND {second_bound})",
                    if *negated { "NOT " } else { "" }
                )
            }
        }
    }
}

impl Display for Select {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str("SELECT ")?;
        match self.predicate {
            SelectPredicate::All => {}
            SelectPredicate::Distinct => f.write_str("DISTINCT ")?,
            SelectPredicate::DistinctRow => f.write_str("DISTINCTROW ")?,
            SelectPredicate::Top { count, percent } => {
                write!(f, "TOP {count} {}", if percent { "PERCENT " } else { "" })?;
            }
        }
        for (index, item) in self.projection.iter().enumerate() {
            if index > 0 {
                f.write_str(", ")?;
            }
            match item {
                Projection::Wildcard(None) => f.write_str("*")?,
                Projection::Wildcard(Some(qualifier)) => write!(f, "{qualifier}.*")?,
                Projection::Field { field, alias } => {
                    field.fmt(f)?;
                    if let Some(alias) = alias {
                        write!(f, " AS {alias}")?;
                    }
                }
            }
        }
        write!(f, " FROM {}", self.from.name)?;
        if let Some(alias) = &self.from.alias {
            write!(f, " AS {alias}")?;
        }
        if let Some(selection) = &self.selection {
            write!(f, " WHERE {selection}")?;
        }
        for (index, key) in self.order_by.iter().enumerate() {
            f.write_str(if index == 0 { " ORDER BY " } else { ", " })?;
            write!(
                f,
                "{} {}",
                key.field,
                if key.descending { "DESC" } else { "ASC" }
            )?;
        }
        f.write_str(";")
    }
}
