use fugue::ir::{Address, AddressSpace, IntoAddress};
use fugue::ir::il::pcode::{Operand, PCode};

use fuguex_state::{State, StateValue};

use crate::types::{Outcome, StepState};

pub trait Interpreter<'space, V: StateValue + 'space> {
    type State: State<Value=V> + 'space;
    type Error: std::error::Error + From<<Self::State as State>::Error> + 'space;

    fn fork(&self) -> Self;
    fn restore(&mut self, other: &Self);

    fn copy(&mut self, source: &Operand<'space>, destination: &Operand<'space>) -> Result<Outcome<'space>, Self::Error>;
    fn load(&mut self, source: &Operand<'space>, destination: &Operand<'space>, space: &'space AddressSpace) -> Result<Outcome<'space>, Self::Error>;
    fn store(&mut self, source: &Operand<'space>, destination: &Operand<'space>, space: &'space AddressSpace) -> Result<Outcome<'space>, Self::Error>;

    fn lift<A>(&mut self, address: A) -> Result<StepState<'space>, Self::Error>
        where A: IntoAddress;
}
