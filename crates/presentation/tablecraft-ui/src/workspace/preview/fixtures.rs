//! Invented presentation data. No captured database records or private images.
use super::super::State;
use crate::workspace::objects::{ObjectKind, ObjectTarget};
use crate::{
    Cell, CellKind, CollectionKind, Column, Data, Database, DisplayControl, Field, FieldProperty,
    FieldPropertyValue, HorizontalAlignment, ParameterSpec, SavedQuery, SavedQueryTarget, Schema,
    Table,
};

use tablecraft_display::{DateRepresentation, NativeFormat, ScalarPresentation};

pub(super) const REVISION: u64 = 1;
pub(super) const PAGE_ROWS: u32 = 100;
pub(super) const TOTAL_ROWS: u32 = 205;
pub(super) const SQL: &str =
    "SELECT TOP 12 CustomerID, CustomerName, City FROM Customers ORDER BY CustomerID;";

pub(super) fn workspace() -> State {
    let mut state = State {
        mock_preview: true,
        path: "Sample Workspace.accdb".into(),
        sql: SQL.into(),
        database: Some(Database {
            title_path: Some("Synthetic workspace/Sample Workspace.accdb".into()),
            document_revision: REVISION,
            title: "Sample Workspace : Database | Sample Workspace.accdb (Access 2016)".into(),
            format: "ACE 16".into(),
            tables: ["Customers", "Orders", "Products", "MSysObjects"]
                .into_iter()
                .map(|name| Table {
                    name: name.into(),
                    local: true,
                    system: name == "MSysObjects",
                    storage: "Local".into(),
                })
                .collect(),
            queries: vec![
                SavedQuery {
                    object_id: 42,
                    name: "Customers by city".into(),
                    system: false,
                },
                SavedQuery {
                    object_id: 43,
                    name: "Annual totals".into(),
                    system: false,
                },
            ],
        }),
        ..Default::default()
    };
    state.dismiss_backstage();
    state
        .prepare_object_open(ObjectTarget {
            document_revision: REVISION,
            kind: ObjectKind::Table {
                name: "Customers".into(),
            },
        })
        .expect("synthetic initial table object");
    state.set_table_data(page("Customers", 0));
    state
}

/// Long, literal object names exercise compact presentation without real files.
pub(super) fn long_names(state: &mut State) {
    let table = "Customer accounts for international correspondence and follow-up";
    if let Some(tab) = state.active_object() {
        state.close_object(&tab);
    }
    let database = state.database.as_mut().expect("synthetic database");
    let name = "International customer and supplier reconciliation workspace — ".repeat(3);
    database.title = format!("{name}2026 : Database | {name}2026.accdb (Access 2016)");
    database.title_path = Some(format!("Synthetic workspace/{name}2026.accdb"));
    database.tables[0].name = table.into();
    database.queries[0].name =
        "Customer account correspondence by region and financial period".into();
    state
        .prepare_object_open(ObjectTarget {
            document_revision: REVISION,
            kind: ObjectKind::Table { name: table.into() },
        })
        .expect("synthetic renamed table");
    state.set_table_data(page(table, 0));
}

pub(super) fn page(table: &str, offset: u32) -> Data {
    let end = offset.saturating_add(PAGE_ROWS).min(TOTAL_ROWS);
    let columns = columns(table);
    let rows = (offset..end).map(|index| row(table, index)).collect();
    Data {
        row_ordinals: None,
        source_revision: Some(REVISION),
        table: table.into(),
        offset,
        columns,
        rows,
        has_more: end < TOTAL_ROWS,
    }
}

fn columns(table: &str) -> Vec<Column> {
    let names: &[(&str, &str)] = match table {
        "Orders" => &[
            ("OrderID", "Order ID"),
            ("CustomerID", "Customer ID"),
            ("OrderDate", "Order date"),
            ("OrderTotal", "Total"),
            ("Shipped", "Shipped"),
        ],
        "Products" => &[
            ("ProductID", "Product ID"),
            ("ProductName", "Product"),
            ("UnitPrice", "Unit price"),
            ("InStock", "In stock"),
        ],
        "MSysObjects" => &[("Id", "Id"), ("Name", "Name"), ("Type", "Type")],
        _ => &[
            ("CustomerID", "Customer ID"),
            ("CustomerName", "Customer"),
            ("Company", "Company"),
            ("City", "City"),
            ("Balance", "Balance"),
            ("Registered", "Registered"),
            ("Active", "Active"),
            ("Categories", "Categories"),
            ("Documents", "Documents"),
            ("Notes", "Notes"),
            ("AnnualTotal", "Annual total"),
        ],
    };
    names
        .iter()
        .map(|(name, caption)| Column {
            name: (*name).into(),
            caption: Some((*caption).into()),
        })
        .collect()
}

fn row(table: &str, index: u32) -> Vec<Cell> {
    match table {
        "Orders" => vec![
            right_aligned((6001 + index).to_string()),
            right_aligned((1001 + index % 24).to_string()),
            date_cell(2026, 5, 14),
            currency_cell(4_250_000),
            boolean(!index.is_multiple_of(3)),
        ],
        "Products" => vec![
            right_aligned((201 + index).to_string()),
            text(["Desk lamp", "Notebook", "Storage box"][index as usize % 3]),
            currency_cell(245_000),
            boolean(!index.is_multiple_of(4)),
        ],
        "MSysObjects" => vec![
            right_aligned((100 + index).to_string()),
            text(format!("SampleObject{}", index + 1)),
            text("1"),
        ],
        _ => {
            let variant = index as usize % 6;
            let name = [
                "Avery Stone",
                "Mina Park",
                "Kenji Sato",
                "李明",
                "김하늘",
                "佐藤 春香",
            ][variant];
            let city = ["Portland", "서울", "東京", "北京", "서울", "京都"][variant];
            vec![
                right_aligned((1001 + index).to_string()),
                text(name),
                text(["Northstar Design", "Cedar Studio", "Harbor Works"][variant % 3]),
                if index == 2 { null() } else { text(city) },
                currency_cell(12_455_000),
                date_cell(2026, 4, 12),
                boolean(!index.is_multiple_of(3)),
                Cell {
                    presentation: None,
                    alignment: HorizontalAlignment::Left,
                    text: "Priority; Newsletter".into(),
                    kind: CellKind::Multivalue {
                        assigned: true,
                        members: 2,
                    },
                },
                Cell {
                    presentation: None,
                    alignment: HorizontalAlignment::Left,
                    text: "1 attachment".into(),
                    kind: CellKind::Attachments {
                        assigned: true,
                        members: 1,
                    },
                },
                Cell {
                    presentation: None,
                    alignment: HorizontalAlignment::Left,
                    text: "Schedule the next account review.".into(),
                    kind: CellKind::VersionHistory {
                        assigned: true,
                        members: 3,
                    },
                },
                Cell {
                    presentation: None,
                    alignment: HorizontalAlignment::Right,
                    text: "$ 14,946.00".into(),
                    kind: CellKind::Scalar {
                        is_null: false,
                        stored_calculation: true,
                        boolean: None,
                        display_control: DisplayControl::Native(109),
                    },
                },
            ]
        }
    }
}

pub(super) fn query() -> Data {
    let mut data = page("Customers", 0);
    data.source_revision = None;
    data.table = "Query results".into();
    data.columns = ["CustomerID", "CustomerName", "City"]
        .into_iter()
        .map(Column::from)
        .collect();
    data.rows.truncate(12);
    data.rows = data
        .rows
        .into_iter()
        .map(|row| {
            row.into_iter()
                .enumerate()
                .filter_map(|(index, cell)| matches!(index, 0 | 1 | 3).then_some(cell))
                .collect()
        })
        .collect();
    data.has_more = false;
    data
}

pub(super) fn saved_target() -> SavedQueryTarget {
    SavedQueryTarget {
        document_revision: REVISION,
        object_id: 42,
        name: "Customers by city".into(),
    }
}

pub(super) fn parameters(target: &SavedQueryTarget) -> Vec<ParameterSpec> {
    if target.object_id != 42 {
        return Vec::new();
    }
    vec![ParameterSpec {
        name: "Enter a city".into(),
        kind: "text".into(),
        label: "TEXT(64)".into(),
        nullable: false,
    }]
}

pub(super) fn contents(kind: CollectionKind) -> Data {
    let (columns, rows): (&[&str], Vec<Vec<Cell>>) = match kind {
        CollectionKind::Attachments => (
            &["FileName", "FileType", "Encoded bytes"],
            vec![vec![
                text("Project brief.pdf"),
                text("application/pdf"),
                text("128,400"),
            ]],
        ),
        CollectionKind::Multivalue => (
            &["Value"],
            vec![vec![text("Priority")], vec![text("Newsletter")]],
        ),
        CollectionKind::VersionHistory => (
            &["Modified", "Value"],
            vec![
                vec![
                    text("2026-05-12 09:30"),
                    text("Schedule the next account review."),
                ],
                vec![
                    text("2026-04-18 14:15"),
                    text("A sample account note was updated."),
                ],
                vec![
                    text("2026-03-02 11:00"),
                    text("The synthetic customer record was created."),
                ],
            ],
        ),
    };
    Data {
        row_ordinals: None,
        source_revision: None,
        table: "Collection contents".into(),
        offset: 0,
        columns: columns.iter().map(|name| Column::from(*name)).collect(),
        rows,
        has_more: false,
    }
}

pub(super) fn schema(table: &str) -> Schema {
    let fields = columns(table)
        .into_iter()
        .enumerate()
        .map(|(number, column)| {
            let (display_type, field_size, native_type, bytes) = match column.name.as_str() {
                "CustomerID" | "OrderID" | "ProductID" => ("AutoNumber", "Long Integer", 4, 4),
                "Balance" | "OrderTotal" | "UnitPrice" | "AnnualTotal" => {
                    ("Currency", "8 bytes", 5, 8)
                }
                "Registered" | "OrderDate" => ("Date/Time", "8 bytes", 8, 8),
                "Active" | "Shipped" | "InStock" => ("Yes/No", "1 bit", 1, 0),
                "Notes" => ("Long Text", "Long Text", 12, 0),
                "Documents" => ("Attachment", "", 18, 4),
                "Categories" => ("Short Text", "64", 18, 4),
                _ => ("Short Text", "120", 10, 240),
            };
            Field {
                name: column.name.clone(),
                number,
                native_type,
                storage_bytes: bytes,
                fixed: native_type != 10 && native_type != 12,
                flags: 1,
                extended_flags: 0,
                scalar_type: match native_type {
                    1 => Some("Boolean".into()),
                    4 => Some("Int32".into()),
                    5 => Some("Currency".into()),
                    8 => Some("DateTime".into()),
                    10 | 12 => Some("Text".into()),
                    _ => None,
                },
                reader_nullable: Some(number != 0),
                decimal_parameters: None,
                complex_type_id: (native_type == 18).then_some(1),
                display_type: Some(display_type.into()),
                field_size: Some(field_size.into()),
                properties: vec![
                    FieldProperty {
                        name: "Description",
                        value: FieldPropertyValue::Text(format!(
                            "Synthetic {} field.",
                            column.label()
                        )),
                    },
                    FieldProperty {
                        name: "Caption",
                        value: FieldPropertyValue::Text(column.label().into()),
                    },
                    FieldProperty {
                        name: "Required",
                        value: FieldPropertyValue::Boolean(number == 0),
                    },
                    FieldProperty {
                        name: "AllowZeroLength",
                        value: FieldPropertyValue::Boolean(false),
                    },
                    FieldProperty {
                        name: "DefaultValue",
                        value: if column.name == "Balance" {
                            FieldPropertyValue::Text("0".into())
                        } else {
                            FieldPropertyValue::Absent
                        },
                    },
                    FieldProperty {
                        name: "ValidationRule",
                        value: FieldPropertyValue::Absent,
                    },
                ],
            }
        })
        .collect();
    Schema {
        table: table.into(),
        declared_rows: TOTAL_ROWS,
        fields,
    }
}

pub(super) fn text(value: impl Into<String>) -> Cell {
    Cell::scalar(value)
}

fn right_aligned(value: impl Into<String>) -> Cell {
    let mut cell = Cell::scalar(value);
    cell.alignment = HorizontalAlignment::Right;
    cell
}

pub(super) fn null() -> Cell {
    Cell {
        presentation: None,
        alignment: HorizontalAlignment::Left,
        text: "NULL".into(),
        kind: CellKind::Scalar {
            is_null: true,
            stored_calculation: false,
            boolean: None,
            display_control: DisplayControl::Native(109),
        },
    }
}

fn boolean(value: bool) -> Cell {
    Cell {
        presentation: None,
        alignment: HorizontalAlignment::Left,
        text: value.to_string(),
        kind: CellKind::Scalar {
            is_null: false,
            stored_calculation: false,
            boolean: Some(value),
            display_control: DisplayControl::Native(106),
        },
    }
}

fn currency_cell(scaled: i64) -> Cell {
    let mut cell = right_aligned(String::new());
    cell.presentation = Some(ScalarPresentation::Currency {
        scaled,
        native_format: NativeFormat::Currency,
    });
    cell
}

fn date_cell(year: i16, month: i8, day: i8) -> Cell {
    let mut cell = right_aligned(String::new());
    cell.presentation = Some(ScalarPresentation::DateTime {
        calendar: jiff::civil::date(year, month, day).at(0, 0, 0, 0),
        representation: DateRepresentation::Extended,
        native_format: NativeFormat::ShortDate,
    });
    cell
}
