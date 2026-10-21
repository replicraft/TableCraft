//! Named and guarded derived-source carriers; neither resolves database content.
use super::{Context, E};
use crate::saved::context::check_name;
use tablecraft_ace::SavedQueryRow;
use tablecraft_core::limits::Resource;
use tablecraft_sql::{
    query::{ReadQuery, Statement},
    select::TableSource,
};

pub(super) struct Source<'a> {
    pub body: Body<'a>,
    pub alias: Option<&'a str>,
}
pub(super) enum Body<'a> {
    Named(&'a str),
    Derived {
        text: &'a str,
        query: Box<ReadQuery>,
    },
}
impl<'a> Source<'a> {
    pub fn read(row: &'a SavedQueryRow, context: &mut Context<'_>) -> Result<Self, E> {
        context.check()?;
        let alias = row.name2();
        if let Some(alias) = alias {
            context.budget.charge(Resource::Steps, alias.len() as u64)?;
            check_name(alias)?;
        }
        let body = match (row.name1(), row.expression()) {
            (Some(name), None) => {
                context.budget.charge(Resource::Steps, name.len() as u64)?;
                check_name(name)?;
                Body::Named(name)
            }
            (None, Some(text)) => {
                let query = context.query(&[text])?;
                if !query.parameters.is_empty() {
                    return Err(E::Unsupported);
                }
                let Statement::Read(query) = query.statement else {
                    return Err(E::Unsupported);
                };
                let guarded = context.select(&["SELECT 0 FROM (", text, "\n)"])?;
                if !matches!(guarded.from.as_slice(), [TableSource::Derived { query: actual, alias: None }] if actual == &query)
                {
                    return Err(E::InvalidMetadata);
                }
                Body::Derived { text, query }
            }
            (Some(_), Some(_)) => return Err(E::Unsupported),
            (None, None) => return Err(E::InvalidMetadata),
        };
        Ok(Self { body, alias })
    }
    pub fn label(&self) -> Option<&str> {
        self.alias.or(match &self.body {
            Body::Named(name) => Some(*name),
            Body::Derived { .. } => None,
        })
    }
}
