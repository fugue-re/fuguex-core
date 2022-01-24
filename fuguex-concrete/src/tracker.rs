use fugue::bytes::Order;
use fugue::ir::il::pcode::Operand;
use fugue::ir::Address;
use fuguex_hooks::types::{Error, HookCBranchAction, HookOutcome, HookStepAction};
use fuguex_machine::StepState;
use fuguex_state::pcode::{Error as PCodeError, PCodeState};
use fuguex_state::{AsState, StateOps};

use crate::hooks::{ClonableHookConcrete, HookConcrete};

use std::marker::PhantomData;

pub struct BranchTracker<S, O, R> {
    next: Address,
    tracked: Vec<(Address, Address, Address)>,
    marker: PhantomData<(S, O, R)>,
}

impl<S, O, R> Default for BranchTracker<S, O, R> {
    fn default() -> Self {
        Self {
            next: Address::from_value(0u64),
            tracked: Vec::default(),
            marker: PhantomData,
        }
    }
}

impl<S, O, R> Clone for BranchTracker<S, O, R> {
    fn clone(&self) -> Self {
        Self {
            next: self.next.clone(),
            tracked: self.tracked.clone(),
            marker: PhantomData,
        }
    }
}

impl<S, O, R> HookConcrete for BranchTracker<S, O, R>
where
    S: AsState<PCodeState<u8, O>> + StateOps<Value = u8> + 'static,
    O: Order + 'static,
    R: 'static,
{
    type State = S;
    type Error = PCodeError;
    type Outcome = R;

    fn hook_architectural_step(
        &mut self,
        _state: &mut Self::State,
        _address: &Address,
        operation: &StepState,
    ) -> Result<HookOutcome<HookStepAction<Self::Outcome>>, Error<Self::Error>> {
        self.next = operation.fallthrough().into();
        Ok(HookStepAction::Pass.into())
    }

    fn hook_cbranch(
        &mut self,
        state: &mut Self::State,
        destination: &Operand,
        _condition: &Operand,
    ) -> Result<HookOutcome<HookCBranchAction<Self::Outcome>>, Error<Self::Error>> {
        let pcode_state = state.state_ref();
        let address = pcode_state.program_counter_value().map_err(Error::state)?;
        let taken = pcode_state.get_address(destination).map_err(Error::state)?;
        let not_taken = self.next.clone();

        self.tracked.push((address, taken, not_taken));

        Ok(HookCBranchAction::Pass.into())
    }
}

impl<S, O, R> ClonableHookConcrete for BranchTracker<S, O, R>
where
    S: AsState<PCodeState<u8, O>> + StateOps<Value = u8> + 'static,
    O: Order + 'static,
    R: 'static,
{
}
