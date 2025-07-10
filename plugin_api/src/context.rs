#[repr(C)]
#[derive(Clone, Debug, Default, Copy)]
pub struct Context {
    width: u16,  // ?
    height: u16, // ?
    left_display_mode: DisplayMode,
    right_display_mode: DisplayMode,
    focus: Focus,
    dual: bool,
    metadata: bool,
    preview: bool,
    is_disabled: bool,
    left_show_hidden: bool,
    right_show_hidden: bool,
    left_sort_kind: SortKind,
    right_sort_kind: SortKind,
    // left_filter_kind
    // right_filter_kind
    // // tab ?
    // /// Last searched string
    // pub search: Search,
    // // pub searched: Search,
    // /// Visited directories
    // pub history: History,
    // /// Users & groups
    // pub users: Users,
    // /// Saved path before entering "CD" mode.
    // /// Used if the cd is canceled
    // pub origin_path: Option<std::path::PathBuf>,
    // pub visual: bool,
}

#[repr(C)]
#[derive(Clone, Debug, Default, Copy)]
pub enum Focus {
    #[default]
    FileLeft,
    MenuLeft,
    FileRight,
    MenuRight,
}

#[repr(C)]
#[derive(Clone, Debug, Default, Copy)]
pub enum DisplayMode {
    #[default]
    Directory,
    Tree,
    Preview,
    Fuzzy,
}

/// Different kind of sort
#[repr(C)]
#[derive(Debug, Clone, Default, Copy)]
enum SortBy {
    #[default]
    /// Directory first
    Kind,
    /// by filename
    File,
    /// by date
    Date,
    /// by size
    Size,
    /// by extension
    Exte,
}

/// Ascending or descending sort
#[repr(C)]
#[derive(Debug, Clone, Default, Copy)]
enum Order {
    #[default]
    /// Ascending order
    Ascending,
    /// Descending order
    Descending,
}

#[repr(C)]
#[derive(Debug, Clone, Default, Copy)]
/// Describe a way of sorting
pub struct SortKind {
    /// The key used to sort the files
    sort_by: SortBy,
    /// Ascending or descending order
    order: Order,
}
