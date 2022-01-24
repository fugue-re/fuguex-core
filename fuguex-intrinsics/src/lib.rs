use fugue::ir::il::pcode::Operand;
use fugue::ir::AddressValue;

use fuguex_state::State;

use std::collections::HashMap;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error<E: std::error::Error + 'static> {
    #[error(transparent)]
    State(E),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl<E> Error<E>
where
    E: std::error::Error + 'static,
{
    pub fn state(error: E) -> Self {
        Self::State(error)
    }
}

#[derive(Clone)]
pub enum IntrinsicAction<O> {
    Pass,
    Branch(AddressValue),
    Halt(O),
}

pub trait IntrinsicBehaviour: dyn_clone::DynClone {
    type Outcome;
    type State: State;

    fn intrinsic(&self) -> &str;

    fn initialise(
        &mut self,
        #[allow(unused)] state: &Self::State,
    ) -> Result<(), Error<<Self::State as State>::Error>> {
        Ok(())
    }

    fn handle_intrinsic(
        &mut self,
        state: &mut Self::State,
        inputs: &[Operand],
        output: Option<&Operand>,
    ) -> Result<IntrinsicAction<Self::Outcome>, Error<<Self::State as State>::Error>>;
}
dyn_clone::clone_trait_object!(<Outcome, State> IntrinsicBehaviour<Outcome=Outcome, State=State> where State: fuguex_state::State);

#[derive(Clone)]
pub struct IntrinsicHandler<O, S: State> {
    handlers: HashMap<String, Box<dyn IntrinsicBehaviour<Outcome = O, State = S>>>,
    default_action: IntrinsicAction<O>,
}

impl<O: Clone, S: State> Default for IntrinsicHandler<O, S> {
    fn default() -> Self {
        Self::new()
    }
}

impl<O: Clone, S: State> IntrinsicHandler<O, S> {
    pub fn new() -> Self {
        Self::new_with(IntrinsicAction::Pass)
    }

    pub fn new_with(default_action: IntrinsicAction<O>) -> Self {
        Self {
            handlers: HashMap::new(),
            default_action,
        }
    }

    pub fn register<IN: IntrinsicBehaviour<Outcome = O, State = S> + 'static>(
        &mut self,
        behaviour: IN,
        state: &S,
    ) -> Result<(), Error<S::Error>> {
        let mut behaviour = behaviour;
        behaviour.initialise(state)?;

        let name = behaviour.intrinsic().to_owned();

        self.handlers.insert(name, Box::new(behaviour));

        Ok(())
    }

    pub fn handle(
        &mut self,
        name: &str,
        state: &mut S,
        inputs: &[Operand],
        output: Option<&Operand>,
    ) -> Result<IntrinsicAction<O>, Error<S::Error>> {
        if let Some(handler) = self.handlers.get_mut(name) {
            handler.handle_intrinsic(state, inputs, output)
        } else {
            Ok(self.default_action.clone())
        }
    }
}
