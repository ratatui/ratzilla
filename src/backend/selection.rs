/// Mouse selection mode shared by backends that support text selection.
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub enum SelectionMode {
    /// Select text linearly, following text flow.
    Linear,
    /// Select a rectangular block of cells.
    #[default]
    Block,
}
