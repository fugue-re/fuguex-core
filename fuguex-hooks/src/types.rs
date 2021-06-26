use thiserror::Error;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HookAction {
    Pass,
    Halt,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HookCBranchAction {
    Pass,
    Flip,
    Halt,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HookCallAction {
    Pass,
    Skip,
    Halt,
}

#[derive(Debug, Error)]
pub enum Error<E: std::error::Error + Send + Sync + 'static> {
    #[error(transparent)]
    State(E),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl<E> Error<E> where E: std::error::Error + Send + Sync + 'static {
    pub fn state(error: E) -> Self {
        Self::State(error)
    }
}
