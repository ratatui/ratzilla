use std::{cell::RefCell, rc::Rc};

thread_local! {
    static HYPERLINKS: RefCell<Vec<HyperlinkRegion>> = const { RefCell::new(Vec::new()) };
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct HyperlinkRegion {
    pub(crate) x: u16,
    pub(crate) y: u16,
    pub(crate) width: u16,
    pub(crate) url: Rc<str>,
}

pub(crate) fn begin_frame() {
    HYPERLINKS.with(|regions| regions.borrow_mut().clear());
}

pub(crate) fn register(region: HyperlinkRegion) {
    HYPERLINKS.with(|regions| regions.borrow_mut().push(region));
}

pub(crate) fn take() -> Vec<HyperlinkRegion> {
    HYPERLINKS.with(|regions| std::mem::take(&mut *regions.borrow_mut()))
}
