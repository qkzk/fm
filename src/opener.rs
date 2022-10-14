use std::collections::HashMap;
use std::process::Command;

#[derive(Clone)]
struct OpenerAssociation {
    config: HashMap<String, OpenerInfo>,
}

impl OpenerAssociation {
    pub fn new() -> Self {
        let config = Self::hard_coded();
        Self { config }
    }

    fn hard_coded() -> HashMap<String, OpenerInfo> {
        let mut association = HashMap::new();
        association.insert("md".to_owned(), OpenerInfo::new("nvim".to_owned(), true));
        association.insert(
            "png".to_owned(),
            OpenerInfo::new("viewnior".to_owned(), false),
        );
        association
    }

    fn update_from_file(filepath: std::path::PathBuf) {}
}

#[derive(Clone)]
struct OpenerInfo {
    opener: String,
    use_term: bool,
}

impl OpenerInfo {
    pub fn new(opener: String, use_term: bool) -> Self {
        Self { opener, use_term }
    }
}

#[derive(Clone)]
pub struct Opener {
    terminal: String,
    openers: OpenerAssociation,
}

impl Opener {
    pub fn new(terminal: String) -> Self {
        Self {
            terminal,
            openers: OpenerAssociation::new(),
        }
    }

    pub fn open(&self, filepath: std::path::PathBuf) {
        let extension = filepath.extension().unwrap().to_str().unwrap();
        if let Some(open_config) = self.openers.config.get(extension) {
            if open_config.use_term {
                self.open_terminal(
                    open_config.opener.clone(),
                    filepath.to_str().unwrap().to_owned(),
                )
            } else {
                self.open_directly(
                    open_config.opener.clone(),
                    filepath.to_str().unwrap().to_owned(),
                )
            }
        }
    }

    fn open_directly(&self, executable: String, filepath: String) {
        execute_in_child(&executable, &vec![&filepath]);
    }

    fn open_terminal(&self, executable: String, filepath: String) {
        execute_in_child(&self.terminal, &vec!["-e", &executable, &filepath]);
    }
}

/// Execute the command in a fork.
fn execute_in_child(exe: &str, args: &Vec<&str>) -> std::process::Child {
    eprintln!("exec exe {}, args {:?}", exe, args);
    Command::new(exe).args(args).spawn().unwrap()
}
