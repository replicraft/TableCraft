//! Draft ownership follows the scratch object, with bounded retained heap accounting.
use super::{PendingKind, QueryDesign};
use crate::workspace::objects::memory::Counter;
use tablecraft_builders::query::{DesignExpression, QueryDesignDraft};

impl QueryDesign {
    pub(in crate::workspace) fn retained_bytes(&self) -> Option<usize> {
        let mut count = Counter::new(size_of::<Self>());
        if let Some(session) = &self.session {
            count.add(size_of_val(session.as_ref()));
            count_draft(&mut count, &session.draft);
            count.vector(&session.metadata);
            for table in &session.metadata {
                count.string(&table.table);
                count.vector(&table.fields);
                for field in &table.fields {
                    count.string(&field.name);
                }
            }
            count.optional(&session.error);
            count.optional(&session.selected_table);
            if let Some(pending) = &session.pending {
                match &pending.kind {
                    PendingKind::Describe { table } => count.string(table),
                    PendingKind::Build { draft, .. } => count_draft(&mut count, draft),
                    PendingKind::Import { sql } | PendingKind::Run { sql } => count.string(sql),
                }
            }
        }
        count.finish()
    }
}

pub(super) fn count_draft(count: &mut Counter, draft: &QueryDesignDraft) {
    count.vector(&draft.sources);
    for source in &draft.sources {
        count.string(&source.table);
        count.optional(&source.alias);
    }
    count.vector(&draft.joins);
    for join in &draft.joins {
        count.string(&join.left.column);
        count.string(&join.right.column);
    }
    count.vector(&draft.columns);
    for column in &draft.columns {
        match &column.expression {
            DesignExpression::Field(field) => count.string(&field.column),
            DesignExpression::Expression(text) => count.string(text),
            DesignExpression::AllFields(_) => {}
        }
        count.optional(&column.alias);
        count.vector(&column.criteria);
        for criterion in &column.criteria {
            count.string(criterion);
        }
    }
}
