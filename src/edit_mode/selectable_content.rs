/// Make a content selectable content for a struct.
/// This trait allows to navigate through a vector of element `content_type`.
/// It implements: `is_empty`, `len`, `next`, `prev`, `selected`.
/// `selected` returns an optional reference to the value.
pub trait SelectableContent<T> {
    fn is_empty(&self) -> bool;
    fn len(&self) -> usize;
    fn next(&mut self);
    fn prev(&mut self);
    fn selected(&self) -> Option<&T>;
    fn index(&self) -> usize;
    fn content(&self) -> &Vec<T>;
    fn attr(&self, index: usize, attr: &tuikit::attr::Attr) -> tuikit::attr::Attr;
}

/// Implement the `SelectableContent` for struct `$struc` with content type `$content_type`.
/// This trait allows to navigate through a vector of element `content_type`.
/// It implements: `is_empty`, `len`, `next`, `prev`, `selected`.
/// `selected` returns an optional reference to the value.
#[macro_export]
macro_rules! impl_selectable_content {
    ($content_type:ident, $struct:ident) => {
        use $crate::edit_mode::SelectableContent;

        /// Implement a selectable content for this struct.
        /// This trait allows to navigate through a vector of element `content_type`.
        /// It implements: `is_empty`, `len`, `next`, `prev`, `selected`.
        /// `selected` returns an optional reference to the value.
        impl SelectableContent<$content_type> for $struct {
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

            /// Returns a reference to the selected content.
            fn selected(&self) -> Option<&$content_type> {
                match self.is_empty() {
                    true => None,
                    false => Some(&self.content[self.index]),
                }
            }

            /// Returns the index of the selected item.
            fn index(&self) -> usize {
                self.index
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
