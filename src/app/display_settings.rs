use crate::config::Settings;
use crate::io::Args;
use crate::io::MIN_WIDTH_FOR_DUAL_PANE;

/// Holds settings about display :
/// - do we display one or two tabs ?
/// - do we display files metadata ?
/// - do we use to second pane to preview files ?
pub struct DisplaySettings {
    /// do we display one or two tabs ?
    pub dual: bool,
    /// do we display all info or only the filenames ?
    pub metadata: bool,
    /// use the second pane to preview
    pub preview: bool,
}

impl DisplaySettings {
    pub fn new(args: &Args, settings: &Settings, width: usize) -> Self {
        Self {
            metadata: Self::parse_display_full(args.simple, settings.full),
            dual: Self::parse_dual_pane(args.dual, settings.dual, width),
            preview: args.preview,
        }
    }

    fn parse_dual_pane(args_dual: Option<bool>, dual_config: bool, width: usize) -> bool {
        if !Self::display_wide_enough(width) {
            return false;
        }
        if let Some(args_dual) = args_dual {
            return args_dual;
        }
        dual_config
    }

    fn parse_display_full(simple_args: Option<bool>, full_config: bool) -> bool {
        if let Some(simple_args) = simple_args {
            return !simple_args;
        }
        full_config
    }

    /// True iff the terminal is wide enough to display two panes
    ///
    /// # Errors
    ///
    /// Fail if the terminal has crashed
    pub fn display_wide_enough(width: usize) -> bool {
        width >= MIN_WIDTH_FOR_DUAL_PANE
    }

    pub fn use_dual_tab(&self, width: usize) -> bool {
        self.dual && Self::display_wide_enough(width)
    }
}
