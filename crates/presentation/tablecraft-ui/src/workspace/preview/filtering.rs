//! Predetermined synthetic memberships illustrate controls, never native results.
use super::{Preview, fixtures};
use crate::{Data, FilterComparison, FilterPublication, FilterTarget, RowOrdinals};

pub(super) enum FilterScene {
    Applied,
    Empty,
    Disabled,
    Error,
}

impl Preview {
    pub(super) fn prepare_filter_scene(&mut self, scene: FilterScene) {
        let enabled = !matches!(scene, FilterScene::Disabled);
        let empty = matches!(scene, FilterScene::Empty);
        let first = fixtures::page("Customers", 0);
        let columns = first.columns.clone();
        self.state
            .table_selection
            .select(self.state.data.as_ref().unwrap(), 0, 1);
        let selection = self
            .state
            .current_filter_selection()
            .expect("synthetic native field");
        let request = self.request();
        assert!(
            self.state
                .begin_filter_selection(request, selection, FilterComparison::Equal)
        );
        let mut rows = Vec::new();
        let mut ordinals = Vec::new();
        let mut matched_rows = 0;
        for offset in (0..fixtures::TOTAL_ROWS).step_by(fixtures::PAGE_ROWS as usize) {
            for (index, row) in fixtures::page("Customers", offset)
                .rows
                .into_iter()
                .enumerate()
            {
                let ordinal = offset + index as u32;
                // The invented fixture repeats this name every sixth row.
                let member = !empty && ordinal.is_multiple_of(6);
                matched_rows += u32::from(member);
                let visible = if enabled {
                    member
                } else {
                    ordinal < fixtures::PAGE_ROWS
                };
                if visible {
                    rows.push(row);
                    ordinals.push(ordinal);
                }
            }
        }
        let target = FilterTarget {
            document_revision: fixtures::REVISION,
            table: "Customers".into(),
            filter_id: request,
        };
        let publication = FilterPublication {
            sort: None,
            filter_count: 1,
            target: target.clone(),
            enabled: true,
            matched_rows,
            source_rows: fixtures::TOTAL_ROWS,
            data: Data {
                source_revision: Some(fixtures::REVISION),
                row_ordinals: Some(RowOrdinals::new(ordinals).unwrap()),
                table: "Customers".into(),
                offset: 0,
                columns,
                rows,
                has_more: !enabled,
            },
        };
        if enabled {
            assert!(self.state.set_filtered_data(request, publication));
        } else {
            // This scene starts with a retained filter already toggled off;
            // no synthetic operation reports a native search completion.
            self.state.install_filtered_view(FilterPublication {
                enabled: false,
                ..publication
            });
        }
        if matches!(scene, FilterScene::Error) {
            let request = self.request();
            assert!(self.state.begin_filter_page(request, target, 0, false));
            self.state
                .fail_filter(request, self.state.localizer.text("filtering-mock-error"));
        }
    }
}

impl Preview {
    pub(super) fn prepare_sorted_scene(&mut self) {
        let columns = fixtures::page("Customers", 0).columns;
        let mut source = Vec::new();
        for offset in (0..fixtures::TOTAL_ROWS).step_by(fixtures::PAGE_ROWS as usize) {
            source.extend(fixtures::page("Customers", offset).rows);
        }
        let mut rows = Vec::new();
        let mut ordinals = Vec::new();
        for (ordinal, row) in source
            .into_iter()
            .enumerate()
            .rev()
            .take(fixtures::PAGE_ROWS as usize)
        {
            rows.push(row);
            ordinals.push(ordinal as u32);
        }
        let filter_id = self.request();
        self.state.install_filtered_view(FilterPublication {
            target: FilterTarget {
                document_revision: fixtures::REVISION,
                table: "Customers".into(),
                filter_id,
            },
            sort: Some(crate::SortSpec {
                column: "CustomerID".into(),
                descending: true,
            }),
            filter_count: 0,
            enabled: false,
            matched_rows: fixtures::TOTAL_ROWS,
            source_rows: fixtures::TOTAL_ROWS,
            data: Data {
                source_revision: Some(fixtures::REVISION),
                row_ordinals: Some(RowOrdinals::from_display_order(ordinals).unwrap()),
                table: "Customers".into(),
                offset: 0,
                columns,
                rows,
                has_more: true,
            },
        });
    }
}
