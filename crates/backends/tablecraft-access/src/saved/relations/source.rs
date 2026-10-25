//! Named and guarded derived-source carriers; neither resolves database content.
use super::{Context, E};
use crate::saved::identifier;
use tablecraft_ace::SavedQueryRow;
use tablecraft_core::limits::Resource;
use tablecraft_sql::{
    query::{ReadQuery, Statement},
    select::TableSource,
};

pub(super) struct Source<'a> {
    pub body: Body<'a>,
    pub alias: Option<&'a str>,
    stored_label: Option<&'a str>,
}

pub(super) enum Body<'a> {
    Named {
        name: &'a str,
        database: Option<&'a str>,
    },
    Derived {
        text: &'a str,
        query: Box<ReadQuery>,
    },
}

impl<'a> Source<'a> {
    pub fn read(row: &'a SavedQueryRow, context: &mut Context<'_>) -> Result<Self, E> {
        context.check()?;
        let stored_label = row.name2().or(row.name1());
        if let Some(alias) = row.name2() {
            context.budget.charge(Resource::Steps, alias.len() as u64)?;
        }
        let alias = row.name2().map(identifier::literal).transpose()?;
        let body = match (row.name1(), row.expression()) {
            (Some(name), database) => {
                context.budget.charge(Resource::Steps, name.len() as u64)?;
                if let Some(database) = database {
                    context
                        .budget
                        .charge(Resource::Steps, database.len() as u64)?;
                }
                if database
                    .is_some_and(|value| value.is_empty() || value.chars().any(char::is_control))
                {
                    return Err(E::Unsupported);
                }
                Body::Named {
                    name: identifier::literal(name)?,
                    database,
                }
            }
            (None, Some(text)) => {
                let (query, text) = context.source_query(text)?;
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
            (None, None) => return Err(E::InvalidMetadata),
        };
        Ok(Self {
            body,
            alias,
            stored_label,
        })
    }

    pub fn label(&self) -> Option<&str> {
        self.alias.or(match &self.body {
            Body::Named { name, .. } => Some(*name),
            Body::Derived { .. } => None,
        })
    }

    pub fn stored_label(&self) -> Option<&str> {
        self.stored_label
    }
}
