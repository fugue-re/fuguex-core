use fugue::bytes::Order;
use fugue::ir::Address;
use fuguex_hooks::types::{Error, HookOutcome};
use fuguex_microx::types::{HookInvalidAccessAction, ViolationSource};
use fuguex_state::{AsState, StateOps};
use fuguex_state::pcode::{PCodeState, Error as PCodeError};
use std::marker::PhantomData;

use crate::hooks::{ClonableHookConcrete, HookConcrete};

pub trait MemoryPolicy {
    type State;
    type Outcome;
    type Value;
    type Error: std::error::Error + Send + Sync;

    fn handle_read(
        &mut self,
        state: &mut Self::State,
        address: &Address,
        size: usize,
    ) -> Result<Option<Vec<Self::Value>>, Self::Error>;
}

pub struct ZeroPolicy<S, O, R>(PhantomData<(S, O, R)>);

impl<S, O, R> Default for ZeroPolicy<S, O, R> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<S, O, R> Clone for ZeroPolicy<S, O, R> {
    fn clone(&self) -> Self {
        Self(PhantomData)
    }
}

impl<S, O, R> MemoryPolicy for ZeroPolicy<S, O, R>
where S: AsState<PCodeState<u8, O>>,
      O: Order {
    type State = S;
    type Outcome = R;
    type Value = u8;
    type Error = PCodeError;

    fn handle_read(&mut self, _state: &mut Self::State, _address: &Address, size: usize) -> Result<Option<Vec<Self::Value>>, PCodeError> {
        Ok(Some(vec![0u8; size]))
        /*
        let state = state.state_mut();

        let addr = u64::from(address);
        let base_addr = u64::from(address) & !0xfffu64;

        let diff = addr + (size as u64) - base_addr;

        let r = 0x1000 - (diff % 0x1000);
        let d = diff / 0x1000;

        let size = (d * 0x1000 + r) as usize;

        // will be zero
        state.memory_mut().static_mapping(
            format!("microx-{:x}", base_addr),
            base_addr,
            size,
        ).ok(); // TODO: handle error
        */
    }
}

#[derive(Debug)]
pub struct PolicyHook<P, S, O>
where
    P: Clone + MemoryPolicy<State=S>,
{
    policy: P,
    marker: PhantomData<(S, O)>,
}

impl<P, S, O> Clone for PolicyHook<P, S, O>
where
    P: Clone + MemoryPolicy<State = S>,
    S: StateOps,
{
    fn clone(&self) -> Self {
        Self {
            policy: self.policy.clone(),
            marker: PhantomData,
        }
    }
}

impl<P, S, O> PolicyHook<P, S, O>
where
    P: Clone + MemoryPolicy<State = S>,
    S: StateOps,
{
    pub fn new(policy: P) -> Self {
        Self {
            policy,
            marker: PhantomData,
        }
    }

    pub fn read_memory(
        &mut self,
        state: &mut S,
        address: &Address,
        size: usize,
    ) -> Result<Option<Vec<P::Value>>, Error<P::Error>> {
        self.policy
            .handle_read(state, address, size)
            .map_err(Error::state)
    }
}

impl<P, S, E, O> HookConcrete for PolicyHook<P, S, O>
where
    P: Clone + MemoryPolicy<State = S, Error = E, Value = u8> + 'static,
    S: StateOps<Value = u8> + 'static,
    E: std::error::Error + Send + Sync + 'static,
    O: 'static,
{
    type State = S;
    type Error = E;
    type Outcome = O;

    #[allow(unused)]
    fn hook_invalid_memory_access(
        &mut self,
        state: &mut Self::State,
        address: &Address,
        size: usize,
        source: ViolationSource,
    ) -> Result<HookOutcome<HookInvalidAccessAction<Self::Outcome, u8>>, Error<Self::Error>> {
        if matches!(source, ViolationSource::Read) {
            if let Some(bytes) = self.policy.handle_read(state, address, size).map_err(Error::state)? {
                Ok(HookInvalidAccessAction::Value(bytes).into())
            } else {
                Ok(HookInvalidAccessAction::Pass.into())
            }
        } else {
            Ok(HookInvalidAccessAction::Skip.into())
        }
    }
}

impl<P, S, O> ClonableHookConcrete for PolicyHook<P, S, O>
where
    P: Clone + MemoryPolicy<State = S, Value = u8> + 'static,
    S: StateOps<Value = u8> + 'static,
    O: 'static,
{
}
