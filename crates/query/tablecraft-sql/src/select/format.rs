use std::fmt::{self, Display, Formatter};

use super::{
    BinaryOperator, Expression, FieldReference, FunctionArgument, Name, Projection, Select,
    SelectPredicate, UnaryOperator, precedence as power,
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

/// Ordinary queries retain their historical fully parenthesized spelling.
/// Declaration annotations instead use the same binding powers as the parser,
/// avoiding artificial nesting inside mandatory CHECK/call/subquery delimiters.
#[derive(Clone, Copy)]
pub(crate) enum ExpressionStyle {
    Canonical,
    Annotation,
}

impl ExpressionStyle {
    pub(crate) fn root(self, expression: &Expression) -> FormattedExpression<'_> {
        self.operand(expression, 0)
    }

    fn operand(self, expression: &Expression, minimum: u8) -> FormattedExpression<'_> {
        FormattedExpression {
            expression,
            style: self,
            minimum,
        }
    }
}

pub(crate) struct FormattedExpression<'a> {
    expression: &'a Expression,
    style: ExpressionStyle,
    minimum: u8,
}

impl Display for Expression {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        ExpressionStyle::Canonical.root(self).fmt(f)
    }
}

fn expression_power(expression: &Expression) -> u8 {
    match expression {
        Expression::Binary { operator, .. } => power::binary(*operator),
        Expression::Unary {
            operator: UnaryOperator::Not,
            ..
        } => power::NOT,
        Expression::Unary { .. } | Expression::Integer(..=-1) => power::UNARY_SIGN,
        Expression::Like { .. }
        | Expression::InQuery { .. }
        | Expression::Quantified { .. }
        | Expression::IsNull { .. }
        | Expression::Between { .. }
        | Expression::InList { .. } => power::COMPARISON,
        _ => u8::MAX,
    }
}

impl Display for FormattedExpression<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let style = self.style;
        let grouped = match style {
            ExpressionStyle::Canonical => matches!(
                self.expression,
                Expression::Unary { .. }
                    | Expression::Binary { .. }
                    | Expression::Like { .. }
                    | Expression::InQuery { .. }
                    | Expression::Quantified { .. }
                    | Expression::IsNull { .. }
                    | Expression::Between { .. }
                    | Expression::InList { .. }
            ),
            ExpressionStyle::Annotation => expression_power(self.expression) < self.minimum,
        };
        if grouped {
            f.write_str("(")?;
        }
        match self.expression {
            Expression::Field(field) => field.fmt(f),
            Expression::Integer(value) => value.fmt(f),
            Expression::Text(value) => write!(f, "'{}'", value.replace('\'', "''")),
            Expression::Number(value) => f.write_str(value),
            Expression::DateLiteral(value) => write!(f, "#{value}#"),
            Expression::GuidLiteral(value) => write!(f, "{{GUID {{{value}}}}}"),
            Expression::Identity => f.write_str("@@IDENTITY"),
            Expression::Call {
                qualifier,
                name,
                arguments,
            } => {
                for component in qualifier {
                    write!(f, "{component}.")?;
                }
                if name
                    .as_str()
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '_' || c == '$')
                {
                    f.write_str(name.as_str())?;
                } else {
                    name.fmt(f)?;
                }
                f.write_str("(")?;
                for (index, argument) in arguments.iter().enumerate() {
                    if index > 0 {
                        f.write_str(", ")?;
                    }
                    match argument {
                        FunctionArgument::Expression(expression) => {
                            style.root(expression).fmt(f)?
                        }
                        FunctionArgument::Wildcard => f.write_str("*")?,
                        FunctionArgument::Missing => {}
                    }
                }
                f.write_str(")")
            }
            Expression::Unary {
                operator,
                expression,
            } => {
                let (operator, minimum) = match operator {
                    UnaryOperator::Plus => ("+", power::UNARY_SIGN),
                    UnaryOperator::Minus => ("-", power::UNARY_SIGN),
                    UnaryOperator::Not => ("NOT", power::NOT),
                };
                // The space also prevents consecutive minus signs becoming a comment.
                write!(f, "{operator} {}", style.operand(expression, minimum))
            }
            Expression::Like {
                expression,
                pattern,
                negated,
            } => write!(
                f,
                "{} {}LIKE {}",
                style.operand(expression, power::COMPARISON),
                if *negated { "NOT " } else { "" },
                style.operand(pattern, power::COMPARISON + 1)
            ),
            Expression::Member {
                expression,
                member,
                bang,
            } => write!(
                f,
                "{}{}{member}",
                style.operand(expression, u8::MAX),
                if *bang { "!" } else { "." }
            ),
            Expression::Subquery(query) | Expression::Exists(query) => {
                if matches!(self.expression, Expression::Exists(_)) {
                    f.write_str("EXISTS ")?;
                }
                f.write_str("(")?;
                query.fmt_with_style(f, style)?;
                f.write_str(")")
            }
            Expression::InQuery {
                expression,
                query,
                negated,
            } => {
                write!(
                    f,
                    "{} {}IN (",
                    style.operand(expression, power::COMPARISON),
                    if *negated { "NOT " } else { "" }
                )?;
                query.fmt_with_style(f, style)?;
                f.write_str(")")
            }
            Expression::Quantified {
                expression,
                operator,
                quantifier,
                query,
            } => {
                write!(
                    f,
                    "{} {operator} {} (",
                    style.operand(expression, power::COMPARISON),
                    match quantifier {
                        super::Quantifier::All => "ALL",
                        super::Quantifier::Any => "ANY",
                        super::Quantifier::Some => "SOME",
                    }
                )?;
                query.fmt_with_style(f, style)?;
                f.write_str(")")
            }
            Expression::Null => f.write_str("NULL"),
            Expression::Boolean(value) => f.write_str(if *value { "TRUE" } else { "FALSE" }),
            Expression::Binary {
                left,
                operator,
                right,
            } => {
                let minimum = power::binary(*operator);
                // The parser associates every binary operator, including ^, leftwards.
                write!(
                    f,
                    "{} {operator} {}",
                    style.operand(left, minimum),
                    style.operand(right, minimum + 1)
                )
            }
            Expression::IsNull {
                expression,
                negated,
            } => write!(
                f,
                "{} IS {}NULL",
                style.operand(expression, power::COMPARISON),
                if *negated { "NOT " } else { "" }
            ),
            Expression::Between {
                expression,
                first_bound,
                second_bound,
                negated,
            } => write!(
                f,
                "{} {}BETWEEN {} AND {}",
                style.operand(expression, power::COMPARISON),
                if *negated { "NOT " } else { "" },
                style.operand(first_bound, power::COMPARISON + 1),
                style.operand(second_bound, power::COMPARISON + 1)
            ),
            Expression::InList {
                expression,
                values,
                negated,
            } => {
                write!(
                    f,
                    "{} {}IN (",
                    style.operand(expression, power::COMPARISON),
                    if *negated { "NOT " } else { "" }
                )?;
                for (index, value) in values.iter().enumerate() {
                    if index > 0 {
                        f.write_str(", ")?;
                    }
                    style.root(value).fmt(f)?;
                }
                f.write_str(")")
            }
        }?;
        if grouped {
            f.write_str(")")?;
        }
        Ok(())
    }
}

pub(crate) fn sort(f: &mut Formatter<'_>, items: &[super::SortKey]) -> fmt::Result {
    sort_with_style(f, items, ExpressionStyle::Canonical)
}

pub(crate) fn sort_with_style(
    f: &mut Formatter<'_>,
    items: &[super::SortKey],
    style: ExpressionStyle,
) -> fmt::Result {
    for (i, key) in items.iter().enumerate() {
        f.write_str(if i == 0 { " ORDER BY " } else { ", " })?;
        write!(
            f,
            "{} {}",
            style.root(&key.expression),
            if key.descending { "DESC" } else { "ASC" }
        )?;
    }
    Ok(())
}
impl Display for super::TableSource {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.fmt_with_style(f, ExpressionStyle::Canonical)
    }
}

impl super::TableSource {
    fn fmt_with_style(&self, f: &mut Formatter<'_>, style: ExpressionStyle) -> fmt::Result {
        match self {
            Self::Derived { query, alias } => {
                f.write_str("(")?;
                query.fmt_with_style(f, style)?;
                f.write_str(")")?;
                if let Some(alias) = alias {
                    write!(f, " AS {alias}")?;
                }
                Ok(())
            }
            Self::Table(table) => {
                if let Some(database) = &table.database {
                    f.write_str("[")?;
                    for character in database.chars() {
                        if character == ']' {
                            f.write_str("]]")?;
                        } else {
                            write!(f, "{character}")?;
                        }
                    }
                    f.write_str("].")?;
                }
                table.name.fmt(f)?;
                if let Some(alias) = &table.alias {
                    write!(f, " AS {alias}")?;
                }
                Ok(())
            }
            Self::Group { source, alias } => {
                f.write_str("(")?;
                source.fmt_with_style(f, style)?;
                f.write_str(")")?;
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
            } => {
                f.write_str("(")?;
                left.fmt_with_style(f, style)?;
                write!(
                    f,
                    " {} JOIN ",
                    match kind {
                        super::JoinKind::Inner => "INNER",
                        super::JoinKind::Left => "LEFT",
                        super::JoinKind::Right => "RIGHT",
                    }
                )?;
                right.fmt_with_style(f, style)?;
                write!(f, " ON {})", style.root(condition))
            }
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
        self.fmt_body_with_style(f, ExpressionStyle::Canonical)
    }

    pub(crate) fn fmt_body_with_style(
        &self,
        f: &mut Formatter<'_>,
        style: ExpressionStyle,
    ) -> fmt::Result {
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
                    style.root(expression).fmt(f)?;
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
            source.fmt_with_style(f, style)?;
        }
        if let Some(external) = &self.external {
            external.fmt(f)?;
        }
        if let Some(selection) = &self.selection {
            write!(f, " WHERE {}", style.root(selection))?;
        }
        for (i, expr) in self.group_by.iter().enumerate() {
            f.write_str(if i == 0 { " GROUP BY " } else { ", " })?;
            style.root(expr).fmt(f)?;
        }
        if let Some(having) = &self.having {
            write!(f, " HAVING {}", style.root(having))?;
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
