/// Context of the application itself.
/// Used to send `Copy` attributes to the plugin.
#[repr(C)]
#[derive(Clone, Debug, Default, Copy)]
pub struct FMContext {
    /// Fm global context
    pub status: StatusContext,
    /// left tab context
    pub left_tab: TabContext,
    /// right tab context
    pub right_tab: TabContext,
}

/// Global context of the fm application.
/// It is `Copy`.
#[repr(C)]
#[derive(Clone, Debug, Default, Copy)]
pub struct StatusContext {
    /// Which window is focused (left or right, menu or file) ?
    pub focus: Focus,
    /// Does the user wants dual mode ?
    pub dual: bool,
    /// Does the user wants metadata ?
    pub metadata: bool,
    /// Is the right tab used to previewing ?
    pub preview: bool,
    /// Is the terminal currently disabled ?
    pub is_disabled: bool,
}

/// Context of the left or right tab
/// It is 'Copy'.
#[repr(C)]
#[derive(Clone, Debug, Default, Copy)]
pub struct TabContext {
    /// Display mode of the file window
    pub display_mode: super::DisplayMode,
    /// Are hidden files displayed ?
    pub show_hidden: bool,
    /// What kind of sort is used ?
    pub sort_kind: SortKind,
}

#[repr(C)]
#[derive(Clone, Debug, Default, Copy)]
pub enum Focus {
    #[default]
    LeftFile,
    LeftMenu,
    RightFile,
    RightMenu,
}

/// Different kind of sort
#[repr(C)]
#[derive(Debug, Clone, Default, Copy)]
pub enum SortBy {
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
pub enum Order {
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
    pub sort_by: SortBy,
    /// Ascending or descending order
    pub order: Order,
}

// non copy informations :

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
