use fugue::bytes::Order;
use fugue::ir::il::pcode::Operand;
use fugue::ir::Address;
use fuguex_hooks::types::{Error, HookCBranchAction, HookOutcome, HookStepAction};
use fuguex_machine::StepState;
use fuguex_state::pcode::{Error as PCodeError, PCodeState};
use fuguex_state::{AsState, StateOps};

use crate::hooks::{ClonableHookConcrete, HookConcrete};

use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

use std::collections::VecDeque;
use std::iter::FromIterator;
use std::marker::PhantomData;

pub struct PathWalker<S, O, R> {
    next: Address,
    walk: VecDeque<(Address, Address)>,
    marker: PhantomData<(S, O, R)>,
}

impl<S, O, R> Default for PathWalker<S, O, R> {
    fn default() -> Self {
        Self {
            next: Address::from_value(0u64),
            walk: VecDeque::default(),
            marker: PhantomData,
        }
    }
}

impl<S, O, R> Clone for PathWalker<S, O, R> {
    fn clone(&self) -> Self {
        Self {
            next: self.next.clone(),
            walk: self.walk.clone(),
            marker: PhantomData,
        }
    }
}

impl<S, O, R> FromIterator<(Address, Address)> for PathWalker<S, O, R> {
    fn from_iter<T: IntoIterator<Item = (Address, Address)>>(iter: T) -> Self {
        Self {
            walk: iter.into_iter().collect(),
            ..Default::default()
        }
    }
}

impl<S, O, R> PathWalker<S, O, R> {
    pub fn push(&mut self, from: Address, to: Address) {
        self.walk.push_back((from, to));
    }
}

impl<S, O, R> HookConcrete for PathWalker<S, O, R>
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
        _destination: &Operand,
        condition: &Operand,
    ) -> Result<HookOutcome<HookCBranchAction<Self::Outcome>>, Error<Self::Error>> {
        let pcode_state = state.state_ref();
        let address = pcode_state.program_counter_value().map_err(Error::state)?;
        let is_taken = pcode_state.get_operand::<u8>(condition).map_err(Error::state)? != 0;

        if matches!(self.walk.front(), Some((expected, _)) if *expected == address) {
            let (_, next) = self.walk.pop_front().unwrap();
            Ok(if next == self.next {
                if is_taken { HookCBranchAction::Flip } else { HookCBranchAction::Pass }
            } else {
                if is_taken { HookCBranchAction::Pass } else { HookCBranchAction::Flip }
            }.into())
        } else {
            Ok(HookCBranchAction::Pass.into())
        }
    }
}

impl<S, O, R> ClonableHookConcrete for PathWalker<S, O, R>
where
    S: AsState<PCodeState<u8, O>> + StateOps<Value = u8> + 'static,
    O: Order + 'static,
    R: 'static,
{
}

pub struct RandomWalker<R: Rng, S, E, O> {
    rand: R,
    marker: PhantomData<(S, E, O)>,
}

impl<B, S, E, O> Clone for RandomWalker<B, S, E, O>
where
    B: Clone + Rng,
{
    fn clone(&self) -> Self {
        Self {
            rand: self.rand.clone(),
            marker: PhantomData,
        }
    }
}

impl<B, S, E, O> RandomWalker<B, S, E, O>
where
    B: Rng,
{
    pub fn new(rand: B) -> RandomWalker<B, S, E, O> {
        RandomWalker {
            rand,
            marker: PhantomData,
        }
    }
}

impl<S, E, O> RandomWalker<SmallRng, S, E, O> {
    pub fn new_small() -> RandomWalker<SmallRng, S, E, O> {
        RandomWalker::new(SmallRng::from_entropy())
    }
}

impl<B, S, E, O> HookConcrete for RandomWalker<B, S, E, O>
where
    B: Rng + 'static,
    S: StateOps + 'static,
    E: std::error::Error + Send + Sync + 'static,
    O: 'static,
{
    type State = S;
    type Error = E;
    type Outcome = O;

    fn hook_cbranch(
        &mut self,
        _state: &mut Self::State,
        _destination: &Operand,
        _condition: &Operand,
    ) -> Result<HookOutcome<HookCBranchAction<Self::Outcome>>, Error<Self::Error>> {
        Ok(if self.rand.gen::<bool>() {
            HookCBranchAction::Flip
        } else {
            HookCBranchAction::Pass
        }
        .into())
    }
}

impl<B, S, E, O> ClonableHookConcrete for RandomWalker<B, S, E, O>
where
    B: Clone + Rng + 'static,
    S: StateOps + 'static,
    E: std::error::Error + Send + Sync + 'static,
    O: 'static,
{
}
