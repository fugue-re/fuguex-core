use thiserror::Error;

pub enum HookAction<R> {
    Pass,
    Halt(R),
}

pub enum HookCBranchAction<R> {
    Pass,
    Flip,
    Halt(R),
}

pub enum HookCallAction<R> {
    Pass,
    Skip,
    Halt(R),
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
