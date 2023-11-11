trait User {
    fn id(&self) -> u32;
    fn name(&self) -> &str;
    fn from_id_and_name(id: u32, name: &str) -> Self;
}

#[derive(Debug, Clone)]
struct Owner {
    id: u32,
    name: String,
}

impl User for Owner {
    fn id(&self) -> u32 {
        self.id
    }

    fn name(&self) -> &str {
        self.name.as_ref()
    }

    fn from_id_and_name(id: u32, name: &str) -> Self {
        Self {
            id,
            name: name.to_owned(),
        }
    }
}

type Group = Owner;

// NOTE: should this be splitted in 2 ?

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
    users: Vec<Owner>,
    groups: Vec<Group>,
}

impl Users {
    /// Search for an id in a _**sorted**_ collection of `Owner`, returns its name.
    fn search(collection: &[Owner], id: u32) -> Option<String> {
        if let Ok(index) = collection
            .iter()
            .map(|pair| pair.id())
            .collect::<Vec<_>>()
            .binary_search(&id)
        {
            return Some(collection[index].name().into());
        }
        None
    }

    /// Name of the user from its uid.
    pub fn get_user_by_uid(&self, uid: u32) -> Option<String> {
        Self::search(&self.users, uid)
    }

    /// Name of the group from its gid.
    pub fn get_group_by_gid(&self, gid: u32) -> Option<String> {
        Self::search(&self.groups, gid)
    }

    // NOTE: can't refactor further since GroupEntry and PasswdEntry don't share common trait
    fn update_users(mut self) -> Self {
        let mut users: Vec<Owner> = pgs_files::passwd::get_all_entries()
            .iter()
            .map(|entry| User::from_id_and_name(entry.uid, &entry.name))
            .collect();
        users.sort_unstable_by_key(|pair| pair.id());
        self.users = users;
        self
    }

    fn update_groups(mut self) -> Self {
        let mut groups: Vec<Group> = pgs_files::group::get_all_entries()
            .iter()
            .map(|entry| Group::from_id_and_name(entry.gid, &entry.name))
            .collect();
        groups.sort_unstable_by_key(|pair| pair.id());
        self.groups = groups;
        self
    }

    /// Creates a default instance and update both users and groups from
    /// `/etc/passwd` and `/etc/group` respectively.
    pub fn new() -> Self {
        Self::default().update_users().update_groups()
    }
}
