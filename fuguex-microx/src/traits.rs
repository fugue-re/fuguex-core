use fugue::ir::Address;

use fuguex_hooks::types::{Error, HookOutcome};
use fuguex_state::StateOps;

use crate::types::{HookInvalidAccessAction, ViolationSource};

pub trait HookInvalidMemoryAccess {
    type State: StateOps;
    type Outcome;

    type Error: std::error::Error + Send + Sync + 'static;

    fn hook_invalid_memory_access(
        &mut self,
        state: &mut Self::State,
        address: &Address,
        size: usize,
        source: ViolationSource,
    ) -> Result<
        HookOutcome<HookInvalidAccessAction<Self::Outcome, <Self::State as StateOps>::Value>>,
        Error<Self::Error>,
    >;
}
