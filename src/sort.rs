use std::cmp::Ordering;

use crate::fileinfo::FileInfo;

/// Different kind of sort
#[derive(Debug, Clone, Default)]
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
#[derive(Debug, Clone, Default)]
enum Order {
    #[default]
    /// Ascending order
    Ascending,
    /// Descending order
    Descending,
}

impl Order {
    fn reverse(&self) -> Self {
        match self {
            Self::Descending => Self::Ascending,
            Self::Ascending => Self::Descending,
        }
    }
}

#[derive(Debug, Clone, Default)]
/// Describe a way of sorting
pub struct SortKind {
    /// The key used to sort the files
    sort_by: SortBy,
    /// Ascending or descending order
    order: Order,
}

impl SortKind {
    /// Updates itself from a given character.
    /// If the character describes a kind of sort, we apply it. (k n m s e -- K N M S E)
    /// If the character is lowercase, we sort by Ascending order, else Descending order.
    /// If the character is 'r' or 'R' we reverse current kind of sort.
    pub fn update_from_char(&mut self, c: char) {
        match c {
            'k' | 'K' => self.sort_by = SortBy::Kind,
            'n' | 'N' => self.sort_by = SortBy::File,
            'm' | 'M' => self.sort_by = SortBy::Date,
            's' | 'S' => self.sort_by = SortBy::Size,
            'e' | 'E' => self.sort_by = SortBy::Exte,
            'r' | 'R' => self.order = self.order.reverse(),
            _ => {
                return;
            }
        }
        if c != 'r' {
            if c.is_uppercase() {
                self.order = Order::Descending
            } else {
                self.order = Order::Ascending
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
        K: Ord,
    {
        slice.sort_by(|a, b| f(a).cmp(f(b)))
    }

    /// Use Higher Rank Trait Bounds
    /// Avoid using slices to sort a collection.
    /// It allows use to use references to `String` (`&str`) instead of cloning the `String`.
    /// Reference: [StackOverflow](https://stackoverflow.com/questions/56105305/how-to-sort-a-vec-of-structs-by-a-string-field)
    /// This version uses a reversed comparaison, allowing a descending sort.
    fn reversed_sort_by_key_hrtb<T, F, K>(slice: &mut [T], f: F)
    where
        F: for<'a> Fn(&'a T) -> &'a K,
        K: Ord,
    {
        slice.sort_by(|a, b| Ordering::reverse(f(a).cmp(f(b))))
    }

    pub fn sort(&self, files: &mut [FileInfo]) {
        if let Order::Ascending = self.order {
            match self.sort_by {
                SortBy::Kind => Self::sort_by_key_hrtb(files, |f| &f.kind_format),
                SortBy::File => Self::sort_by_key_hrtb(files, |f| &f.filename),
                SortBy::Date => Self::sort_by_key_hrtb(files, |f| &f.system_time),
                SortBy::Size => Self::sort_by_key_hrtb(files, |f| &f.file_size),
                SortBy::Exte => Self::sort_by_key_hrtb(files, |f| &f.extension),
            }
        } else {
            match self.sort_by {
                SortBy::Kind => Self::reversed_sort_by_key_hrtb(files, |f| &f.kind_format),
                SortBy::File => Self::reversed_sort_by_key_hrtb(files, |f| &f.filename),
                SortBy::Date => Self::reversed_sort_by_key_hrtb(files, |f| &f.system_time),
                SortBy::Size => Self::reversed_sort_by_key_hrtb(files, |f| &f.file_size),
                SortBy::Exte => Self::reversed_sort_by_key_hrtb(files, |f| &f.extension),
            }
        }
    }
}
