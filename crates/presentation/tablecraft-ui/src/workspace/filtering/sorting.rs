//! Native sorting changes a complete issued view, retaining physical identities.
use super::{Operation, State};
use crate::datasheet::SortHeader;
use crate::localization::tr;
use crate::workspace::ribbon::{CommandSize, Icon, command};
use crate::workspace::{Action, DocumentView};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SortTarget {
    pub document_revision: u64,
    pub table: String,
    pub filter_id: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SortSpec {
    pub column: String,
    pub descending: bool,
}

impl State {
    pub fn table_sort(&self) -> Option<SortSpec> {
        self.active_filter_target()?;
        self.table_filter.view.as_ref()?.sort.clone()
    }

    pub fn table_filter_count(&self) -> u32 {
        if self.active_filter_target().is_some() {
            self.table_filter
                .view
                .as_ref()
                .map_or(0, |view| view.filter_count)
        } else {
            0
        }
    }

    pub fn current_sort_target(&self) -> Option<SortTarget> {
        let data = self.data.as_ref()?;
        let target = SortTarget {
            document_revision: data.source_revision?,
            table: data.table.clone(),
            filter_id: self.active_filter_target().map(|target| target.filter_id),
        };
        let known_view = data.row_ordinals.is_none() || target.filter_id.is_some();
        (known_view && self.filter_source_current(target.document_revision, &target.table))
            .then_some(target)
    }

    pub fn can_sort_view(
        &self,
        target: &SortTarget,
        sort: Option<&SortSpec>,
        enabled: bool,
    ) -> bool {
        if !self.filter_controls_ready()
            || self.current_sort_target().as_ref() != Some(target)
            || self.table_filter_enabled() != enabled
        {
            return false;
        }
        match sort {
            Some(sort) => self
                .data
                .as_ref()
                .is_some_and(|data| data.columns.iter().any(|column| column.name == sort.column)),
            None => self.table_sort().is_some(),
        }
    }

    pub fn begin_sort_view(
        &mut self,
        request_id: u64,
        target: SortTarget,
        sort: Option<SortSpec>,
        enabled: bool,
    ) -> bool {
        if !self.can_sort_view(&target, sort.as_ref(), enabled) {
            return false;
        }
        let column = sort.as_ref().map(|sort| sort.column.clone());
        self.begin_filter_operation(
            request_id,
            Operation::Sort {
                target,
                sort,
                enabled,
            },
        );
        if let Some(column) = column
            && let Some(pending) = &mut self.table_filter.pending
        {
            pending.column = Some(column);
        }
        true
    }

    pub(in crate::workspace) fn native_sort_header(&self) -> Option<SortHeader> {
        if self.view != DocumentView::Table {
            return None;
        }
        Some(SortHeader {
            target: self.current_sort_target()?,
            commands_enabled: self.filter_controls_ready(),
            enabled: self.table_filter_enabled(),
            sort: self.table_sort(),
        })
    }

    pub(in crate::workspace) fn sort_commands(
        &mut self,
        ui: &mut egui::Ui,
        actions: &mut Vec<Action>,
    ) {
        let target = self.current_sort_target();
        let column = self.data.as_ref().and_then(|data| {
            if self.total_selection_active() {
                return None;
            }
            let (_, column) = self.table_selection.position()?;
            Some(data.columns.get(column)?.name.clone())
        });
        let enabled = self.table_filter_enabled();
        for (descending, icon, key) in [
            (false, Icon::Ascending, "ribbon-ascending"),
            (true, Icon::Descending, "ribbon-descending"),
        ] {
            let sort = column.as_ref().map(|column| SortSpec {
                column: column.clone(),
                descending,
            });
            let ready = target
                .as_ref()
                .zip(sort.as_ref())
                .is_some_and(|(target, sort)| self.can_sort_view(target, Some(sort), enabled));
            if command(ui, icon, &tr(ui, key), ready, CommandSize::Small)
                .on_hover_text(tr(ui, "filtering-sort-help"))
                .clicked()
                && let (Some(target), Some(sort)) = (&target, sort)
            {
                actions.push(Action::SortView {
                    target: target.clone(),
                    sort: Some(sort),
                    enabled,
                });
            }
        }
        let ready = target
            .as_ref()
            .is_some_and(|target| self.can_sort_view(target, None, enabled));
        if command(
            ui,
            Icon::RemoveSort,
            &tr(ui, "ribbon-remove-sort"),
            ready,
            CommandSize::Small,
        )
        .on_hover_text(tr(ui, "filtering-remove-sort-help"))
        .clicked()
            && let Some(target) = target
        {
            actions.push(Action::SortView {
                target,
                sort: None,
                enabled,
            });
        }
    }
}
