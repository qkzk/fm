use std::iter::{Chain, Skip, Take};
use std::slice::Iter;

use ratatui::style::Style;

// TODO pick a more telling name. `Selectable` doesn't say what it does.
/// Allow selection of a element and basic navigation.
/// Its implementation is mostly made by the macro [`crate::impl_selectable`]
/// which allows to manipulate all sort of content in a common manner.
/// It simplifies a lot the creation of menus for any action and is used everywhere.
pub trait Selectable {
    /// True iff the content is empty
    fn is_empty(&self) -> bool;
    /// Number of element in content
    fn len(&self) -> usize;
    /// Select next element in content
    fn next(&mut self);
    /// Select previous element in content
    fn prev(&mut self);
    /// Current index of selected element.
    /// 0 if the content is empty (I know, could be an option)
    fn index(&self) -> usize;
    /// set the index to the value if possible
    fn set_index(&mut self, index: usize);
    /// true if the selected element is the last of content
    fn selected_is_last(&self) -> bool;
}

/// Allow access to a content element of any type.
/// It allows to access the selected element, the whole content,
/// to push new elements and to get the style of an element for display.
///
/// Its implementation should be done using the [`crate::impl_content`] macro
/// which allows to manipulate any kind of content.
/// It's used for almost every menu or list of things, as long as the whole content
/// is known at some point.
///
/// Major exception is [`crate::modes::FuzzyFinder`] which _doesn't store_ the matches.
pub trait Content<T>: Selectable {
    /// Returns the selected element if content isn't empty
    fn selected(&self) -> Option<&T>;
    /// Reference to the content as a vector.
    fn content(&self) -> &Vec<T>;
    /// add an element to the content
    fn push(&mut self, t: T);
    /// [`ratatui::style::Style`] used to display an element
    fn style(&self, index: usize, style: &Style) -> Style;
}

/// Returns a reference to itself as a `[std::path::Path]`.
/// Usefull for different kind of strings (`&str` or `String`).
pub trait ToPath {
    fn to_path(&self) -> &std::path::Path;
}

/// Iterate over line from current index to bottom then from top to current index.
///
/// Useful when going to next match in search results
pub trait IndexToIndex<T> {
    /// Iterate over line from current index to bottom then from top to current index.
    ///
    /// Useful when going to next match in search results
    fn index_to_index(&self) -> Chain<Skip<Iter<T>>, Take<Iter<T>>>;
}
/// Implement the `SelectableContent` for struct `$struc` with content type `$content_type`.
/// This trait allows to navigate through a vector of element `content_type`.
/// It implements: `is_empty`, `len`, `next`, `prev`, `selected`.
/// `selected` returns an optional reference to the value.
#[macro_export]
macro_rules! impl_selectable {
    ($struct:ident) => {
        use $crate::modes::Selectable;

        /// Implement a selectable content for this struct.
        /// This trait allows to navigate through a vector of element `content_type`.
        /// It implements: `is_empty`, `len`, `next`, `prev`, `selected`.
        /// `selected` returns an optional reference to the value.
        impl Selectable for $struct {
            /// True if the content is empty.
            fn is_empty(&self) -> bool {
                self.content.is_empty()
            }

            /// The size of the content.
            fn len(&self) -> usize {
                self.content.len()
            }

            /// Select the prev item.
            fn prev(&mut self) {
                if self.is_empty() {
                    self.index = 0
                } else if self.index > 0 {
                    self.index -= 1;
                } else {
                    self.index = self.len() - 1
                }
            }

            /// Select the next item.
            fn next(&mut self) {
                if self.is_empty() {
                    self.index = 0;
                } else {
                    self.index = (self.index + 1) % self.len()
                }
            }

            /// Returns the index of the selected item.
            fn index(&self) -> usize {
                self.index
            }

            /// Set the index to a new value if the value is below the length.
            fn set_index(&mut self, index: usize) {
                if index < self.len() {
                    self.index = index;
                }
            }

            fn selected_is_last(&self) -> bool {
                return self.index() + 1 == self.len();
            }
        }
    };
}

/// Implement an iterator from next index of content to the same index,
/// starting back from 0 when the last element is reached.
/// It's used to search an element in content below current and
/// then from the first index to the current index.
#[macro_export]
macro_rules! impl_index_to_index {
    ($content_type:ident, $struct:ident) => {
        use std::iter::{Chain, Enumerate, Skip, Take};
        use std::slice::Iter;
        use $crate::modes::IndexToIndex;

        impl IndexToIndex<$content_type> for $struct {
            /// Iterate over line from current index to bottom then from top to current index.
            ///
            /// Useful when going to next match in search results
            fn index_to_index(
                &self,
            ) -> Chain<Skip<Iter<$content_type>>, Take<Iter<$content_type>>> {
                let index = self.index;
                let elems = self.content();
                elems.iter().skip(index + 1).chain(elems.iter().take(index))
            }
        }
    };
}

/// Implement the `SelectableContent` for struct `$struc` with content type `$content_type`.
/// This trait allows to navigate through a vector of element `content_type`.
/// It implements: `is_empty`, `len`, `next`, `prev`, `selected`.
/// `selected` returns an optional reference to the value.
#[macro_export]
macro_rules! impl_content {
    ($struct:ident, $content_type:ident) => {
        use $crate::modes::Content;

        /// Implement a selectable content for this struct.
        /// This trait allows to navigate through a vector of element `content_type`.
        /// It implements: `is_empty`, `len`, `next`, `prev`, `selected`.
        /// `selected` returns an optional reference to the value.
        impl Content<$content_type> for $struct {
            /// Returns a reference to the selected content.
            fn selected(&self) -> Option<&$content_type> {
                match self.is_empty() {
                    true => None,
                    false => Some(&self.content[self.index]),
                }
            }

            /// A reference to the content.
            fn content(&self) -> &Vec<$content_type> {
                &self.content
            }

            /// Reverse the received effect if the index match the selected index.
            fn style(&self, index: usize, style: &ratatui::style::Style) -> ratatui::style::Style {
                let mut style = *style;
                if index == self.index() {
                    style.add_modifier |= ratatui::style::Modifier::REVERSED;
                }
                style
            }

            /// Push a new element at the end of content
            fn push(&mut self, element: $content_type) {
                self.content.push(element)
            }
        }
    };
}
