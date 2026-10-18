use std::fmt::{self, Display, Formatter};

use super::{
    BinaryOperator, Expression, FieldReference, FunctionArgument, Name, Projection, Select,
    SelectPredicate, UnaryOperator,
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
            Self::Add => "+",
            Self::Subtract => "-",
            Self::Multiply => "*",
            Self::Divide => "/",
            Self::IntegerDivide => "\\",
            Self::Modulo => "MOD",
            Self::Power => "^",
            Self::Concatenate => "&",
            Self::Xor => "XOR",
            Self::Equivalence => "EQV",
            Self::Implication => "IMP",
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
            Self::Number(value) => f.write_str(value),
            Self::DateLiteral(value) => write!(f, "#{value}#"),
            Self::Call { name, arguments } => {
                if name
                    .as_str()
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '_')
                {
                    f.write_str(name.as_str())?;
                } else {
                    name.fmt(f)?;
                }
                f.write_str("(")?;
                for (i, arg) in arguments.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    match arg {
                        FunctionArgument::Expression(expr) => expr.fmt(f)?,
                        FunctionArgument::Wildcard => f.write_str("*")?,
                        FunctionArgument::Missing => {}
                    }
                }
                f.write_str(")")
            }
            Self::Unary {
                operator,
                expression,
            } => write!(
                f,
                "({} {expression})",
                match operator {
                    UnaryOperator::Plus => "+",
                    UnaryOperator::Minus => "-",
                    UnaryOperator::Not => "NOT",
                }
            ),
            Self::Like {
                expression,
                pattern,
                negated,
            } => write!(
                f,
                "({expression} {}LIKE {pattern})",
                if *negated { "NOT " } else { "" }
            ),
            Self::Member {
                expression,
                member,
                bang,
            } => write!(f, "{expression}{}{member}", if *bang { "!" } else { "." }),
            Self::Subquery(query) | Self::Exists(query) => {
                if matches!(self, Self::Exists(_)) {
                    f.write_str("EXISTS ")?;
                }
                f.write_str("(")?;
                query.fmt_sql(f)?;
                f.write_str(")")
            }
            Self::InQuery {
                expression,
                query,
                negated,
            } => {
                write!(
                    f,
                    "({expression} {}IN (",
                    if *negated { "NOT " } else { "" }
                )?;
                query.fmt_sql(f)?;
                f.write_str("))")
            }
            Self::Quantified {
                expression,
                operator,
                quantifier,
                query,
            } => {
                write!(
                    f,
                    "({expression} {operator} {} (",
                    match quantifier {
                        super::Quantifier::All => "ALL",
                        super::Quantifier::Any => "ANY",
                        super::Quantifier::Some => "SOME",
                    }
                )?;
                query.fmt_sql(f)?;
                f.write_str("))")
            }
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
            Self::InList {
                expression,
                values,
                negated,
            } => {
                write!(
                    f,
                    "({expression} {}IN (",
                    if *negated { "NOT " } else { "" }
                )?;
                for (index, value) in values.iter().enumerate() {
                    if index > 0 {
                        f.write_str(", ")?;
                    }
                    value.fmt(f)?;
                }
                f.write_str("))")
            }
        }
    }
}

pub(crate) fn sort(f: &mut Formatter<'_>, items: &[super::SortKey]) -> fmt::Result {
    for (i, key) in items.iter().enumerate() {
        f.write_str(if i == 0 { " ORDER BY " } else { ", " })?;
        write!(
            f,
            "{} {}",
            key.expression,
            if key.descending { "DESC" } else { "ASC" }
        )?;
    }
    Ok(())
}
impl Display for super::TableSource {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Derived { query, alias } => {
                f.write_str("(")?;
                query.fmt_sql(f)?;
                f.write_str(")")?;
                if let Some(alias) = alias {
                    write!(f, " AS {alias}")?;
                }
                Ok(())
            }
            Self::Table(table) => {
                table.name.fmt(f)?;
                if let Some(alias) = &table.alias {
                    write!(f, " AS {alias}")?;
                }
                Ok(())
            }
            Self::Group { source, alias } => {
                write!(f, "({source})")?;
                if let Some(alias) = alias {
                    write!(f, " AS {alias}")?;
                }
                Ok(())
            }
            Self::Join {
                left,
                kind,
                right,
                condition,
            } => write!(
                f,
                "({left} {} JOIN {right} ON {condition})",
                match kind {
                    super::JoinKind::Inner => "INNER",
                    super::JoinKind::Left => "LEFT",
                    super::JoinKind::Right => "RIGHT",
                }
            ),
        }
    }
}
impl Display for super::ExternalDatabase {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, " IN '{}'", self.database.replace('\'', "''"))?;
        if let Some(connection) = &self.connection {
            if self.connection_bracketed {
                write!(f, " [{}]", connection.replace(']', "]]"))?;
            } else {
                write!(f, " '{}'", connection.replace('\'', "''"))?;
            }
        }
        Ok(())
    }
}
impl Select {
    pub(crate) fn fmt_body(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str("SELECT ")?;
        match self.predicate {
            SelectPredicate::All => {}
            SelectPredicate::Distinct => f.write_str("DISTINCT ")?,
            SelectPredicate::DistinctRow => f.write_str("DISTINCTROW ")?,
            SelectPredicate::Top { count, percent }
            | SelectPredicate::DistinctTop { count, percent }
            | SelectPredicate::DistinctRowTop { count, percent } => {
                match self.predicate {
                    SelectPredicate::DistinctTop { .. } => f.write_str("DISTINCT ")?,
                    SelectPredicate::DistinctRowTop { .. } => f.write_str("DISTINCTROW ")?,
                    _ => {}
                }
                write!(f, "TOP {count} {}", if percent { "PERCENT " } else { "" })?;
            }
        }
        for (i, item) in self.projection.iter().enumerate() {
            if i > 0 {
                f.write_str(", ")?;
            }
            match item {
                Projection::Wildcard(None) => f.write_str("*")?,
                Projection::Wildcard(Some(name)) => write!(f, "{name}.*")?,
                Projection::Field { field, alias } => {
                    field.fmt(f)?;
                    if let Some(alias) = alias {
                        write!(f, " AS {alias}")?;
                    }
                }
                Projection::Expression { expression, alias } => {
                    expression.fmt(f)?;
                    if let Some(alias) = alias {
                        write!(f, " AS {alias}")?;
                    }
                }
            }
        }
        if let Some(into) = &self.into {
            write!(f, " INTO {into}")?;
            if let Some(external) = &self.into_external {
                external.fmt(f)?;
            }
        }
        if !self.from.is_empty() {
            f.write_str(" FROM ")?;
        }
        for (i, source) in self.from.iter().enumerate() {
            if i > 0 {
                f.write_str(", ")?;
            }
            source.fmt(f)?;
        }
        if let Some(external) = &self.external {
            external.fmt(f)?;
        }
        if let Some(selection) = &self.selection {
            write!(f, " WHERE {selection}")?;
        }
        for (i, expr) in self.group_by.iter().enumerate() {
            f.write_str(if i == 0 { " GROUP BY " } else { ", " })?;
            expr.fmt(f)?;
        }
        if let Some(having) = &self.having {
            write!(f, " HAVING {having}")?;
        }
        Ok(())
    }
}
impl Display for Select {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.fmt_body(f)?;
        sort(f, &self.order_by)?;
        if self.owner_access {
            f.write_str(" WITH OWNERACCESS OPTION")?;
        }
        f.write_str(";")
    }
}
