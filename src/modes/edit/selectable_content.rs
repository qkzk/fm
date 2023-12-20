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
}

pub trait Content<T>: Selectable {
    /// Returns the selected element if content isn't empty
    fn selected(&self) -> Option<&T>;
    /// Reference to the content as a vector.
    fn content(&self) -> &Vec<T>;
    /// [`tuikit::attr:Attr`] used to display an element
    fn attr(&self, index: usize, attr: &tuikit::attr::Attr) -> tuikit::attr::Attr;
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
        }
    };
}

/// Implement the `SelectableContent` for struct `$struc` with content type `$content_type`.
/// This trait allows to navigate through a vector of element `content_type`.
/// It implements: `is_empty`, `len`, `next`, `prev`, `selected`.
/// `selected` returns an optional reference to the value.
#[macro_export]
macro_rules! impl_content {
    ($content_type:ident, $struct:ident) => {
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
            fn attr(&self, index: usize, attr: &tuikit::attr::Attr) -> tuikit::attr::Attr {
                let mut attr = *attr;
                if index == self.index() {
                    attr.effect |= tuikit::attr::Effect::REVERSE;
                }
                attr
            }
        }
    };
}
