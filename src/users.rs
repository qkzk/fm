/// Users and Groups of current Unix system.
/// It requires `/etc/passwd` and `/etc/group` to be at their usual place.
///
/// Holds two vectors, one for users, one for group.
/// Each vector is a pair of `(u32, String)`, for uid, username and gid, groupname respectively.
/// Those vectors are read from `/etc/passwd` and from `/etc/group` directly.
/// It also provides two methods allowing to access the name from uid or gid.
///
/// Both users and groups use vectors which are sorted by their first element (uid/gid).
/// It allows use to use bisection (binary search) to find the correct name.
/// Cloning should be easy.
#[derive(Clone, Debug, Default)]
pub struct Users {
    users: Vec<(u32, String)>,
    groups: Vec<(u32, String)>,
}

impl Users {
    /// Name of the user from its uid.
    pub fn get_user_by_uid(&self, uid: u32) -> Option<String> {
        if let Ok(index) = self
            .users
            .iter()
            .map(|pair| pair.0)
            .collect::<Vec<_>>()
            .binary_search(&uid)
        {
            return Some(self.users[index].1.to_owned());
        }
        None
    }

    /// Name of the group from its gid.
    pub fn get_group_by_gid(&self, gid: u32) -> Option<String> {
        if let Ok(index) = self
            .groups
            .iter()
            .map(|pair| pair.0)
            .collect::<Vec<_>>()
            .binary_search(&gid)
        {
            return Some(self.groups[index].1.to_owned());
        }
        None
    }

    fn update_users(mut self) -> Self {
        let users = pgs_files::passwd::get_all_entries();
        let mut pairs: Vec<(u32, String)> = users
            .iter()
            .map(|entry| (entry.uid, entry.name.to_owned()))
            .collect();
        pairs.sort_unstable_by_key(|pair| pair.0);
        self.users = pairs;
        self
    }

    fn update_groups(mut self) -> Self {
        let users = pgs_files::group::get_all_entries();
        let mut pairs: Vec<(u32, String)> = users
            .iter()
            .map(|entry| (entry.gid, entry.name.to_owned()))
            .collect();
        pairs.sort_unstable_by_key(|pair| pair.0);
        self.groups = pairs;
        self
    }

    /// Creates a default instance and update both users and groups from
    /// `/etc/passwd` and `/etc/group` respectively.
    pub fn new() -> Self {
        Self::default().update_users().update_groups()
    }
}
