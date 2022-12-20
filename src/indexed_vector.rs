pub trait IndexedVector<T> {
    fn is_empty(&self) -> bool;
    fn len(&self) -> usize;
    fn next(&mut self);
    fn prev(&mut self);
    fn selected(&self) -> Option<&T>;
}
