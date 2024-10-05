use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct Users {
    users: HashMap<u32, String>,
    groups: HashMap<u32, String>,
}

impl Default for Users {
    /// Creates both hashmaps of uid:user and gid:group read from `/etc/passwd` and `/etc/groups`.
    fn default() -> Self {
        let mut users = Self {
            users: HashMap::new(),
            groups: HashMap::new(),
        };
        users.update();
        users
    }
}

impl Users {
    pub fn only_users() -> Self {
        let mut users = Self {
            users: HashMap::new(),
            groups: HashMap::new(),
        };
        users.update_users();
        users
    }

    /// Refresh the users from `/etc/passwd` and `/etc/groups`
    pub fn update(&mut self) {
        self.update_users();
        self.update_groups();
    }

    fn update_users(&mut self) {
        self.users = pgs_files::passwd::get_all_entries()
            .iter()
            .map(|entry| (entry.uid, entry.name.to_owned()))
            .collect();
    }

    fn update_groups(&mut self) {
        self.groups = pgs_files::group::get_all_entries()
            .iter()
            .map(|entry| (entry.gid, entry.name.to_owned()))
            .collect();
    }

    /// Name of the user from its uid.
    pub fn get_user_by_uid(&self, uid: u32) -> Option<&String> {
        self.users.get(&uid)
    }

    /// Name of the group from its gid.
    pub fn get_group_by_gid(&self, gid: u32) -> Option<&String> {
        self.groups.get(&gid)
    }
}
