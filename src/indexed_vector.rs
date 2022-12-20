pub trait IndexedVector<T> {
    fn is_empty(&self) -> bool;
    fn len(&self) -> usize;
    fn next(&mut self);
    fn prev(&mut self);
    fn selected(&self) -> Option<&T>;
}

#[macro_export]
macro_rules! impl_indexed_vector {
    ($t:ident, $u:ident) => {
        use crate::indexed_vector::IndexedVector;

        impl IndexedVector<$t> for $u {
            fn is_empty(&self) -> bool {
                self.content.is_empty()
            }

            fn len(&self) -> usize {
                self.content.len()
            }

            fn next(&mut self) {
                if self.is_empty() {
                    self.index = 0
                } else if self.index > 0 {
                    self.index -= 1;
                } else {
                    self.index = self.len() - 1
                }
            }

            fn prev(&mut self) {
                if self.is_empty() {
                    self.index = 0;
                } else {
                    self.index = (self.index + 1) % self.len()
                }
            }
            fn selected(&self) -> Option<&$t> {
                Some(&self.content[self.index])
            }
        }
    };
}
