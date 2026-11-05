//! Original colored TableCraft vectors, with explicit theme/disabled artwork.
use egui::{Image, Ui};

#[derive(Clone, Copy)]
pub(crate) enum Icon {
    Application,
    Run,
    AddTables,
    FolderOpen,
    Settings,
    Info,
    ArrowLeft,
    Undo,
    Redo,
    Table,
    Attachment,
    TableEdit,
    Paste,
    Cut,
    Copy,
    PaintBrush,
    Ascending,
    Descending,
    Filter,
    Selection,
    Advanced,
    ToggleFilter,
    RemoveSort,
    Refresh,
    Add,
    Save,
    Delete,
    Sum,
    Spelling,
    MoreRecords,
    Search,
    Replace,
    GoTo,
    Cursor,
    Bold,
    Italic,
    Underline,
    TextColor,
    Fill,
    Zoom,
    Key,
    Validation,
    Lookups,
    MacroCreate,
    MacroRename,
    Relationships,
    Properties,
    Indexes,
    Dependencies,
    Builder,
    InsertRows,
    DeleteRows,
}

#[derive(Clone, Copy)]
enum Variant {
    Light,
    Dark,
    DisabledLight,
    DisabledDark,
}

macro_rules! source {
    ($name:literal, $variant:literal) => {
        (
            concat!("bytes://tablecraft/original/", $name, "-", $variant, ".svg"),
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../../assets/ui/icons/tablecraft/",
                $name,
                "-",
                $variant,
                ".svg"
            ))
            .as_slice(),
        )
    };
}

macro_rules! asset {
    ($name:literal, $variant:expr) => {
        match $variant {
            Variant::Light => source!($name, "light"),
            Variant::Dark => source!($name, "dark"),
            Variant::DisabledLight => source!($name, "disabled-light"),
            Variant::DisabledDark => source!($name, "disabled-dark"),
        }
    };
}

impl Icon {
    pub(crate) fn image(self, ui: &Ui, size: f32) -> Image<'static> {
        let variant = match (ui.ctx().theme(), ui.is_enabled()) {
            (egui::Theme::Light, true) => Variant::Light,
            (egui::Theme::Dark, true) => Variant::Dark,
            (egui::Theme::Light, false) => Variant::DisabledLight,
            (egui::Theme::Dark, false) => Variant::DisabledDark,
        };
        let (uri, bytes) = match self {
            Self::Application => asset!("application", variant),
            Self::Run => asset!("run", variant),
            Self::AddTables => asset!("add_tables", variant),
            Self::FolderOpen => asset!("folder_open", variant),
            Self::Settings => asset!("settings", variant),
            Self::Info => asset!("info", variant),
            Self::ArrowLeft => asset!("arrow_left", variant),
            Self::Undo => asset!("undo", variant),
            Self::Redo => asset!("redo", variant),
            Self::Table => asset!("table", variant),
            Self::Attachment => asset!("attachment", variant),
            Self::TableEdit => asset!("table_edit", variant),
            Self::Paste => asset!("paste", variant),
            Self::Cut => asset!("cut", variant),
            Self::Copy => asset!("copy", variant),
            Self::PaintBrush => asset!("paint_brush", variant),
            Self::Ascending => asset!("ascending", variant),
            Self::Descending => asset!("descending", variant),
            Self::Filter => asset!("filter", variant),
            Self::Selection => asset!("selection", variant),
            Self::Advanced => asset!("advanced_filter", variant),
            Self::ToggleFilter => asset!("toggle_filter", variant),
            Self::RemoveSort => asset!("remove_sort", variant),
            Self::Refresh => asset!("refresh", variant),
            Self::Add => asset!("add", variant),
            Self::Save => asset!("save", variant),
            Self::Delete => asset!("delete", variant),
            Self::Sum => asset!("sum", variant),
            Self::Spelling => asset!("spelling", variant),
            Self::MoreRecords => asset!("more_records", variant),
            Self::Search => asset!("search", variant),
            Self::Replace => asset!("replace", variant),
            Self::GoTo => asset!("go_to", variant),
            Self::Cursor => asset!("cursor", variant),
            Self::Bold => asset!("bold", variant),
            Self::Italic => asset!("italic", variant),
            Self::Underline => asset!("underline", variant),
            Self::TextColor => asset!("text_color", variant),
            Self::Fill => asset!("fill", variant),
            Self::Zoom => asset!("zoom", variant),
            Self::Key => asset!("key", variant),
            Self::Validation => asset!("validation", variant),
            Self::Lookups => asset!("lookups", variant),
            Self::MacroCreate => asset!("macro_create", variant),
            Self::MacroRename => asset!("macro_rename", variant),
            Self::Relationships => asset!("relationships", variant),
            Self::Properties => asset!("properties", variant),
            Self::Indexes => asset!("indexes", variant),
            Self::Dependencies => asset!("dependencies", variant),
            Self::Builder => asset!("builder", variant),
            Self::InsertRows => asset!("insert_rows", variant),
            Self::DeleteRows => asset!("delete_rows", variant),
        };
        Image::from_bytes(uri, bytes).fit_to_exact_size(egui::vec2(size, size))
    }

    #[cfg(test)]
    const ALL: [Self; 52] = [
        Self::Application,
        Self::Run,
        Self::AddTables,
        Self::FolderOpen,
        Self::Settings,
        Self::Info,
        Self::ArrowLeft,
        Self::Undo,
        Self::Redo,
        Self::Table,
        Self::Attachment,
        Self::TableEdit,
        Self::Paste,
        Self::Cut,
        Self::Copy,
        Self::PaintBrush,
        Self::Ascending,
        Self::Descending,
        Self::Filter,
        Self::Selection,
        Self::Advanced,
        Self::ToggleFilter,
        Self::RemoveSort,
        Self::Refresh,
        Self::Add,
        Self::Save,
        Self::Delete,
        Self::Sum,
        Self::Spelling,
        Self::MoreRecords,
        Self::Search,
        Self::Replace,
        Self::GoTo,
        Self::Cursor,
        Self::Bold,
        Self::Italic,
        Self::Underline,
        Self::TextColor,
        Self::Fill,
        Self::Zoom,
        Self::Key,
        Self::Validation,
        Self::Lookups,
        Self::MacroCreate,
        Self::MacroRename,
        Self::Relationships,
        Self::Properties,
        Self::Indexes,
        Self::Dependencies,
        Self::Builder,
        Self::InsertRows,
        Self::DeleteRows,
    ];
}

#[cfg(test)]
mod tests;
