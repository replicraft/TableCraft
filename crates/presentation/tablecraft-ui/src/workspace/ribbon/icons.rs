//! Curated Fluent System Icons; source revision and MIT notice live with assets.
use egui::{Color32, Image};

#[derive(Clone, Copy)]
pub(in crate::workspace) enum Icon {
    Table,
    TableEdit,
    Paste,
    Cut,
    Copy,
    PaintBrush,
    Ascending,
    Descending,
    Filter,
    Sort,
    Refresh,
    Add,
    Save,
    Delete,
    Sum,
    Spelling,
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
    Flash,
    Relationships,
    Properties,
    Indexes,
}

macro_rules! asset {
    ($name:literal) => {
        (
            concat!("bytes://tablecraft/fluent/", $name, ".svg"),
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../../assets/ui/icons/fluent/",
                $name,
                ".svg"
            )),
        )
    };
}

impl Icon {
    pub(in crate::workspace) fn image(self, color: Color32, size: f32) -> Image<'static> {
        let (uri, bytes): (&'static str, &'static [u8]) = match self {
            Self::Table => asset!("table"),
            Self::TableEdit => asset!("table_edit"),
            Self::Paste => asset!("clipboard_paste"),
            Self::Cut => asset!("cut"),
            Self::Copy => asset!("copy"),
            Self::PaintBrush => asset!("paint_brush"),
            Self::Ascending => asset!("text_sort_ascending"),
            Self::Descending => asset!("text_sort_descending"),
            Self::Filter => asset!("filter"),
            Self::Sort => asset!("arrow_sort"),
            Self::Refresh => asset!("arrow_sync"),
            Self::Add => asset!("add"),
            Self::Save => asset!("save"),
            Self::Delete => asset!("delete"),
            Self::Sum => asset!("autosum"),
            Self::Spelling => asset!("text_grammar_checkmark"),
            Self::Search => asset!("search"),
            Self::Replace => asset!("arrow_swap"),
            Self::GoTo => asset!("arrow_right"),
            Self::Cursor => asset!("cursor"),
            Self::Bold => asset!("text_bold"),
            Self::Italic => asset!("text_italic"),
            Self::Underline => asset!("text_underline"),
            Self::TextColor => asset!("text_color"),
            Self::Fill => asset!("paint_bucket"),
            Self::Zoom => asset!("zoom_in"),
            Self::Key => asset!("key"),
            Self::Flash => asset!("flash"),
            Self::Relationships => asset!("table_link"),
            Self::Properties => asset!("panel_right"),
            Self::Indexes => asset!("list"),
        };
        Image::from_bytes(uri, bytes)
            .fit_to_exact_size(egui::vec2(size, size))
            .tint(color)
    }
}
