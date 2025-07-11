use std::cmp::Ordering;

use crate::modes::FileInfo;

/// Different kind of sort
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

impl std::fmt::Display for SortBy {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let sort_by = match &self {
            Self::Exte => "Exte",
            Self::Date => "Date",
            Self::File => "File",
            Self::Size => "Size",
            Self::Kind => "Kind",
        };
        write!(f, "{sort_by}")
    }
}

/// Ascending or descending sort
#[derive(Debug, Clone, Default, Copy)]
enum Order {
    #[default]
    /// Ascending order
    Ascending,
    /// Descending order
    Descending,
}

impl Order {
    const fn reverse(self) -> Self {
        match self {
            Self::Descending => Self::Ascending,
            Self::Ascending => Self::Descending,
        }
    }
}

#[derive(Debug, Clone, Default, Copy)]
/// Describe a way of sorting
pub struct SortKind {
    /// The key used to sort the files
    sort_by: SortBy,
    /// Ascending or descending order
    order: Order,
}

impl SortKind {
    /// Default kind of sort for a tree view.
    /// Since files are in reverse order, we need to sort in descending order.
    #[must_use]
    pub const fn tree_default() -> Self {
        Self {
            sort_by: SortBy::Kind,
            order: Order::Descending,
        }
    }

    /// Updates itself from a given character.
    /// If the character describes a kind of sort, we apply it. (k n m s e -- K N M S E)
    /// If the character is lowercase, we sort by Ascending order, else Descending order.
    /// If the character is 'r' or 'R' we reverse current kind of sort.
    pub fn update_from_char(&mut self, c: char) {
        match c.to_ascii_uppercase() {
            'K' => self.sort_by = SortBy::Kind,
            'N' => self.sort_by = SortBy::File,
            'M' => self.sort_by = SortBy::Date,
            'S' => self.sort_by = SortBy::Size,
            'E' => self.sort_by = SortBy::Exte,
            'R' => self.order = self.order.reverse(),
            _ => {
                return;
            }
        }
        if c != 'r' {
            if c.is_uppercase() {
                self.order = Order::Descending;
            } else {
                self.order = Order::Ascending;
            }
        }
    }
    /// Use Higher Rank Trait Bounds
    /// Avoid using slices to sort a collection.
    /// It allows use to use references to `String` (`&str`) instead of cloning the `String`.
    /// Reference: [StackOverflow](https://stackoverflow.com/questions/56105305/how-to-sort-a-vec-of-structs-by-a-string-field)
    fn sort_by_key_hrtb<T, F, K>(slice: &mut [T], f: F)
    where
        F: for<'a> Fn(&'a T) -> &'a K,
        K: Ord + ?Sized,
    {
        slice.sort_unstable_by(|a, b| f(a).cmp(f(b)));
    }

    /// Use Higher Rank Trait Bounds
    /// Avoid using slices to sort a collection.
    /// It allows use to use references to `String` (`&str`) instead of cloning the `String`.
    /// Reference: [StackOverflow](https://stackoverflow.com/questions/56105305/how-to-sort-a-vec-of-structs-by-a-string-field)
    /// This version uses a reversed comparaison, allowing a descending sort.
    fn reversed_sort_by_key_hrtb<T, F, K>(slice: &mut [T], f: F)
    where
        F: for<'a> Fn(&'a T) -> &'a K,
        K: Ord + ?Sized,
    {
        slice.sort_unstable_by(|a, b| Ordering::reverse(f(a).cmp(f(b))));
    }

    // A second version should take 2 parameters.
    // 1. the way to access the data depending on T where files: &mut [T],
    // 2. a closure returning the correct data.

    /// Sort a collection of file depending of enum variants.
    #[rustfmt::skip]
    pub fn sort(&self, files: &mut [FileInfo]) {
        if matches!(self.order, Order::Ascending) {
            match self.sort_by {
                SortBy::Kind => files.sort_unstable_by(|a, b| natord::compare(&a.kind_format(), &b.kind_format())),
                SortBy::File => files.sort_unstable_by(|a, b| natord::compare(&a.filename, &b.filename)),
                SortBy::Date => Self::sort_by_key_hrtb(files, |f| &f.system_time),
                SortBy::Size => Self::sort_by_key_hrtb(files, |f| &f.true_size),
                SortBy::Exte => Self::sort_by_key_hrtb(files, |f| &f.extension),
            }
        } else {
            match self.sort_by {
                SortBy::Kind => files.sort_unstable_by(|a, b| natord::compare(&b.kind_format(), &a.kind_format())),
                SortBy::File => files.sort_unstable_by(|a, b| natord::compare(&b.filename, &a.filename)),
                SortBy::Date => Self::reversed_sort_by_key_hrtb(files, |f| &f.system_time),
                SortBy::Size => Self::reversed_sort_by_key_hrtb(files, |f| &f.true_size),
                SortBy::Exte => Self::reversed_sort_by_key_hrtb(files, |f| &f.extension),
            }
        }
    }
}

impl std::fmt::Display for SortKind {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let sort_order = match &self.order {
            Order::Ascending => "↓",
            Order::Descending => "↑",
        };
        write!(f, "{sort_by} {sort_order}", sort_by = &self.sort_by)
    }
}
