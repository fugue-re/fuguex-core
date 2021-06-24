use std::marker::PhantomData;

use fugue::ir::IntoAddress;
use fugue::ir::il::pcode::{PCode, PCodeOp};
use fuguex_state::StateValue;

use crate::traits::Interpreter;
use crate::types::{Branch, BranchOutcome, Outcome, StepOutcome, StepState};

pub struct Machine<'space, V: StateValue + 'space, I: Interpreter<'space, V>> {
    interpreter: I,
    marker: PhantomData<&'space V>,
}

impl<'space, V, I> From<I> for Machine<'space, V, I>
where V: StateValue + 'space,
      I: Interpreter<'space, V> {
    fn from(interpreter: I) -> Self {
        Self {
            interpreter,
            marker: PhantomData,
        }
    }
}

impl<'space, V, I> Machine<'space, V, I>
where V: StateValue + 'space,
      I: Interpreter<'space, V> {
    pub fn step<A>(&mut self, address: A) -> Result<StepOutcome<'space>, I::Error>
    where A: IntoAddress {
        let mut step_state = self.interpreter.lift(address)?;

        while let Some(op) = step_state.current() {
            let action = match op {
                PCodeOp::Copy { ref source, ref destination } => {
                    self.interpreter.copy(source, destination)
                },
                PCodeOp::Load { ref source, ref destination, space } => {
                    self.interpreter.load(source, destination, space)
                },
                PCodeOp::Store { ref source, ref destination, space } => {
                    self.interpreter.store(source, destination, space)
                },
                _ => unimplemented!("to do"),
            }?;

            match action {
                Outcome::Halt => {
                    return Ok(StepOutcome::Halt)
                },
                Outcome::Branch(ref branch) => if let BranchOutcome::Global(address) = step_state.branch(branch) {
                    return Ok(StepOutcome::Branch(address))
                } else { // local
                    continue
                },
            }
        }

        Ok(StepOutcome::Branch(step_state.fallthrough()))
    }
}
