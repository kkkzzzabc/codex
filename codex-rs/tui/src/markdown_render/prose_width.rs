//! Session preference; callers opt in when rendering assistant prose.
#[cfg(not(test))]
static WIDTH: std::sync::RwLock<Option<usize>> = std::sync::RwLock::new(None);
#[cfg(test)]
thread_local! { static WIDTH: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) }; }
pub(crate) fn init(width: Option<usize>) {
    #[cfg(not(test))]
    {
        *WIDTH
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = width;
    }
    #[cfg(test)]
    WIDTH.set(width);
}
pub(crate) fn current() -> Option<usize> {
    #[cfg(not(test))]
    {
        *WIDTH
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
    #[cfg(test)]
    {
        WIDTH.get()
    }
}
