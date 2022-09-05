use thiserror::Error;
use fugue::ir::{
    Address,
};
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

pub enum HookStepAction<R> {
    Pass,
    Skip,
    Halt(R),
    Branch((u32, Address)),     // (Priority, Address), priority: higher has more priority
}

// If the hook change will create a step state in a new address
// The state_change flage shall be set
pub struct HookOutcome<A> {
    pub action: A,
    pub state_changed: bool,
}

impl<A> From<A> for HookOutcome<A> {
    fn from(action: A) -> Self {
        Self { action, state_changed: false }
    }
}

impl<A> HookOutcome<A> {
    pub fn state_changed(self, changed: bool) -> Self {
        Self { state_changed: changed, ..self }
    }
}

#[derive(Debug, Error)]
pub enum Error<E: std::error::Error + Send + Sync + 'static> {
    #[error(transparent)]
    State(E),
    #[error("Hook Error")]
    Hook(E),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl<E> Error<E> where E: std::error::Error + Send + Sync + 'static {
    pub fn state(error: E) -> Self {
        Self::State(error)
    }
}
