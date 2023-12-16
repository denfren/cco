//! visitor pattern helpers
mod visit_traversals;
pub use visit_traversals::VisitTraversalsMut;

/// Visitor that visits is subjects mutably
pub trait VisitMut<T> {
    fn visit_mut(&mut self, value: &mut T);
}

// blanket impl for FnMut
impl<T, F> VisitMut<T> for F
where
    F: FnMut(&mut T),
{
    fn visit_mut(&mut self, value: &mut T) {
        self(value)
    }
}
