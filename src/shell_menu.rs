use crate::{
    fm_error::{FmError, FmResult},
    impl_selectable_content,
    opener::{execute_in_child_without_output, execute_in_child_without_output_with_path},
    status::Status,
};

#[derive(Clone)]
pub struct ShellMenu {
    pub content: Vec<String>,
    index: usize,
}

impl Default for ShellMenu {
    fn default() -> Self {
        let index = 0;
        let content = vec![
            "lazygit".to_owned(),
            "ncdu".to_owned(),
            "htop".to_owned(),
            "btop".to_owned(),
            "glances".to_owned(),
            "shell".to_owned(),
        ];
        Self { content, index }
    }
}

impl ShellMenu {
    pub fn execute(&self, status: &Status) -> FmResult<()> {
        match self.content[self.index].as_str() {
            "lazygit" => Self::require_cwd_and_command(status, "lazygit"),
            "ncdu" => Self::require_cwd_and_command(status, "ncdu"),
            "htop" => Self::simple(status, "htop"),
            "btop" => Self::simple(status, "btop"),
            "glances" => Self::simple(status, "glances"),
            "shell" => Self::require_cwd(status),
            _ => Ok(()),
        }
    }

    fn require_cwd_and_command(status: &Status, command: &str) -> FmResult<()> {
        let tab = status.selected_non_mut();
        let path = tab
            .directory_of_selected()?
            .to_str()
            .ok_or_else(|| FmError::custom("event_shell", "Couldn't parse the directory"))?;
        execute_in_child_without_output(&status.opener.terminal, &vec!["-d", path, "-e", command])?;
        Ok(())
    }

    fn simple(status: &Status, command: &str) -> FmResult<()> {
        execute_in_child_without_output(&status.opener.terminal, &vec!["-e", command])?;
        Ok(())
    }

    fn _simple_with_args(status: &Status, args: Vec<&str>) -> FmResult<()> {
        execute_in_child_without_output(&status.opener.terminal, &args)?;
        Ok(())
    }

    fn require_cwd(status: &Status) -> FmResult<()> {
        let tab = status.selected_non_mut();
        let path = tab.directory_of_selected()?;
        execute_in_child_without_output_with_path(&status.opener.terminal, path, None)?;
        Ok(())
    }
}

impl_selectable_content!(String, ShellMenu);
