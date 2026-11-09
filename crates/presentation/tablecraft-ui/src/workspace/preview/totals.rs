//! Inventory-backed totals snapshots; no arithmetic over visible fixture records.
use super::{Preview, fixtures};
use crate::workspace::totals::Choice;
use crate::{
    Action, Cell, HorizontalAlignment, TotalAggregate as Aggregate, TotalSpec, TotalsData,
    TotalsDescription, TotalsField, TotalsTarget,
};
use tablecraft_display::{NativeFormat, ScalarPresentation};

impl Preview {
    pub(super) fn mock_describe_totals(&mut self, target: TotalsTarget) {
        let request = self.request();
        if !self.state.begin_totals_description(&target, request) {
            return;
        }
        let fields = self
            .state
            .data
            .as_ref()
            .expect("synthetic native source")
            .columns
            .iter()
            .map(|column| {
                let aggregates = match column.name.as_str() {
                    "AnnualTotal" | "Categories" | "Documents" | "Notes" => Vec::new(),
                    "Registered" | "OrderDate" => vec![
                        Aggregate::Count,
                        Aggregate::Average,
                        Aggregate::Minimum,
                        Aggregate::Maximum,
                    ],
                    "CustomerID" | "OrderID" | "ProductID" | "Balance" | "OrderTotal"
                    | "UnitPrice" => vec![
                        Aggregate::Count,
                        Aggregate::Sum,
                        Aggregate::Average,
                        Aggregate::Minimum,
                        Aggregate::Maximum,
                        Aggregate::StandardDeviation,
                        Aggregate::Variance,
                    ],
                    _ => vec![Aggregate::Count],
                };
                TotalsField {
                    // NO_CLONE: Totals metadata owns independent Column declaration while
                    // original table Data remains live for display and request validation.
                    column: column.clone(),
                    aggregates,
                }
            })
            .collect();
        self.state
            .set_totals_description(request, TotalsDescription { target, fields });
    }

    pub(super) fn mock_compute_totals(&mut self, target: TotalsTarget, specs: Vec<TotalSpec>) {
        let request = self.request();
        if self.state.begin_totals_compute(request, &target, specs) {
            self.state.fail_totals(
                request,
                self.state.localizer.text("totals-mock-unavailable"),
            );
        }
    }

    pub(super) fn prepare_totals_scene(&mut self, error: bool) {
        self.state.toggle_totals_visibility();
        let Some(Action::DescribeTotals { target }) = self.state.take_totals_request() else {
            unreachable!("synthetic Totals row requests metadata only");
        };
        // NO_CLONE: mock_describe_totals transfers its target into retained TotalsDescription;
        // subsequent explicit choices and compute actions need original target.
        self.mock_describe_totals(target.clone());
        // NO_CLONE: First Choice owns exact totals target independently of second Choice and
        // subsequent compute/result; retained for action validation.
        // NO_CLONE: Second Choice owns exact target; original remains needed for compute/result
        // and optional error retry.
        self.state.apply_total_choices(vec![
            Choice {
                target: target.clone(),
                column: "CustomerID".into(),
                aggregate: Some(Aggregate::Count),
            },
            Choice {
                target: target.clone(),
                column: "Balance".into(),
                aggregate: Some(Aggregate::Sum),
            },
        ]);
        let Some(Action::ComputeTotals { specs, .. }) = self.state.take_totals_request() else {
            unreachable!("explicit synthetic aggregate choices request a result");
        };
        let request = self.request();
        // NO_CLONE: Pending compute owns exact selected specs, independently of later result
        // data and optional error retry. The result publisher compares complete specs.
        assert!(
            self.state
                .begin_totals_compute(request, &target, specs.clone())
        );
        let mut count = Cell::scalar("205");
        count.alignment = HorizontalAlignment::Right;
        // Inventory constants for the complete invented fixture, never computed
        // by inspecting the currently visible page or a captured database.
        let amount = Cell {
            text: "255327.5000".into(),
            alignment: HorizontalAlignment::Right,
            presentation: Some(ScalarPresentation::Currency {
                scaled: 2_553_275_000,
                native_format: NativeFormat::Currency,
            }),
            kind: count.kind,
        };
        // NO_CLONE: TotalsData owns target while optional later error-compute request retains
        // original; exact matching does not permit dropping source identity.
        // NO_CLONE: Published result retains spec snapshot; original is used for optional later
        // failing compute to retain previous results. Pending compute has its own snapshot too.
        assert!(self.state.set_totals_result(
            request,
            TotalsData {
                target: target.clone(),
                specs: specs.clone(),
                cells: vec![count, amount],
                source_rows: fixtures::TOTAL_ROWS,
                view_rows: fixtures::TOTAL_ROWS
            }
        ));
        if error {
            let request = self.request();
            assert!(self.state.begin_totals_compute(request, &target, specs));
            self.state.fail_totals(
                request,
                self.state.localizer.text("totals-mock-unavailable"),
            );
        }
    }
}
