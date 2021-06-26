use std::sync::Arc;

use fugue::ir::{AddressSpace, IntoAddress};
use fugue::ir::il::pcode::Operand;

use fuguex_state::State;

use crate::types::{Branch, Outcome, StepState};

pub trait Interpreter {
    type State: State;
    type Error: std::error::Error + From<<Self::State as State>::Error>;

    fn fork(&self) -> Self;
    fn restore(&mut self, other: &Self);

    fn copy(&mut self, source: &Operand, destination: &Operand) -> Result<Outcome, Self::Error>;
    fn load(&mut self, source: &Operand, destination: &Operand, space: Arc<AddressSpace>) -> Result<Outcome, Self::Error>;
    fn store(&mut self, source: &Operand, destination: &Operand, space: Arc<AddressSpace>) -> Result<Outcome, Self::Error>;

    fn branch(&mut self, destination: &Operand) -> Result<Outcome, Self::Error>;
    fn cbranch(&mut self, destination: &Operand, condition: &Operand) -> Result<Outcome, Self::Error>;
    fn ibranch(&mut self, destination: &Operand) -> Result<Outcome, Self::Error>;

    fn call(&mut self, destination: &Operand) -> Result<Outcome, Self::Error>;
    fn icall(&mut self, destination: &Operand) -> Result<Outcome, Self::Error>;
    fn intrinsic(&mut self, name: &str, operands: &[Operand], result: Option<&Operand>) -> Result<Outcome, Self::Error>;
    fn return_(&mut self, destination: &Operand) -> Result<Outcome, Self::Error>;

    fn int_eq(&mut self, destination: &Operand, operand1: &Operand, operand2: &Operand) -> Result<Outcome, Self::Error>;
    fn int_not_eq(&mut self, destination: &Operand, operand1: &Operand, operand2: &Operand) -> Result<Outcome, Self::Error>;
    fn int_less(&mut self, destination: &Operand, operand1: &Operand, operand2: &Operand) -> Result<Outcome, Self::Error>;
    fn int_less_eq(&mut self, destination: &Operand, operand1: &Operand, operand2: &Operand) -> Result<Outcome, Self::Error>;
    fn int_sless(&mut self, destination: &Operand, operand1: &Operand, operand2: &Operand) -> Result<Outcome, Self::Error>;
    fn int_sless_eq(&mut self, destination: &Operand, operand1: &Operand, operand2: &Operand) -> Result<Outcome, Self::Error>;

    fn int_zext(&mut self, destination: &Operand, operand: &Operand) -> Result<Outcome, Self::Error>;
    fn int_sext(&mut self, destination: &Operand, operand: &Operand) -> Result<Outcome, Self::Error>;

    fn int_add(&mut self, destination: &Operand, operand1: &Operand, operand2: &Operand) -> Result<Outcome, Self::Error>;
    fn int_sub(&mut self, destination: &Operand, operand1: &Operand, operand2: &Operand) -> Result<Outcome, Self::Error>;
    fn int_carry(&mut self, destination: &Operand, operand1: &Operand, operand2: &Operand) -> Result<Outcome, Self::Error>;
    fn int_scarry(&mut self, destination: &Operand, operand1: &Operand, operand2: &Operand) -> Result<Outcome, Self::Error>;
    fn int_sborrow(&mut self, destination: &Operand, operand1: &Operand, operand2: &Operand) -> Result<Outcome, Self::Error>;

    fn int_neg(&mut self, destination: &Operand, operand: &Operand) -> Result<Outcome, Self::Error>;
    fn int_not(&mut self, destination: &Operand, operand: &Operand) -> Result<Outcome, Self::Error>;

    fn int_xor(&mut self, destination: &Operand, operand1: &Operand, operand2: &Operand) -> Result<Outcome, Self::Error>;
    fn int_and(&mut self, destination: &Operand, operand1: &Operand, operand2: &Operand) -> Result<Outcome, Self::Error>;
    fn int_or(&mut self, destination: &Operand, operand1: &Operand, operand2: &Operand) -> Result<Outcome, Self::Error>;
    fn int_left_shift(&mut self, destination: &Operand, operand1: &Operand, operand2: &Operand) -> Result<Outcome, Self::Error>;
    fn int_right_shift(&mut self, destination: &Operand, operand1: &Operand, operand2: &Operand) -> Result<Outcome, Self::Error>;
    fn int_sright_shift(&mut self, destination: &Operand, operand1: &Operand, operand2: &Operand) -> Result<Outcome, Self::Error>;

    fn int_mul(&mut self, destination: &Operand, operand1: &Operand, operand2: &Operand) -> Result<Outcome, Self::Error>;
    fn int_div(&mut self, destination: &Operand, operand1: &Operand, operand2: &Operand) -> Result<Outcome, Self::Error>;
    fn int_sdiv(&mut self, destination: &Operand, operand1: &Operand, operand2: &Operand) -> Result<Outcome, Self::Error>;
    fn int_rem(&mut self, destination: &Operand, operand1: &Operand, operand2: &Operand) -> Result<Outcome, Self::Error>;
    fn int_srem(&mut self, destination: &Operand, operand1: &Operand, operand2: &Operand) -> Result<Outcome, Self::Error>;

    fn bool_not(&mut self, destination: &Operand, operand: &Operand) -> Result<Outcome, Self::Error>;
    fn bool_xor(&mut self, destination: &Operand, operand1: &Operand, operand2: &Operand) -> Result<Outcome, Self::Error>;
    fn bool_and(&mut self, destination: &Operand, operand1: &Operand, operand2: &Operand) -> Result<Outcome, Self::Error>;
    fn bool_or(&mut self, destination: &Operand, operand1: &Operand, operand2: &Operand) -> Result<Outcome, Self::Error>;

    fn float_eq(&mut self, destination: &Operand, operand1: &Operand, operand2: &Operand) -> Result<Outcome, Self::Error>;
    fn float_not_eq(&mut self, destination: &Operand, operand1: &Operand, operand2: &Operand) -> Result<Outcome, Self::Error>;
    fn float_less(&mut self, destination: &Operand, operand1: &Operand, operand2: &Operand) -> Result<Outcome, Self::Error>;
    fn float_less_eq(&mut self, destination: &Operand, operand1: &Operand, operand2: &Operand) -> Result<Outcome, Self::Error>;

    fn float_is_nan(&mut self, destination: &Operand, operand: &Operand) -> Result<Outcome, Self::Error>;

    fn float_add(&mut self, destination: &Operand, operand1: &Operand, operand2: &Operand) -> Result<Outcome, Self::Error>;
    fn float_div(&mut self, destination: &Operand, operand1: &Operand, operand2: &Operand) -> Result<Outcome, Self::Error>;
    fn float_mul(&mut self, destination: &Operand, operand1: &Operand, operand2: &Operand) -> Result<Outcome, Self::Error>;
    fn float_sub(&mut self, destination: &Operand, operand1: &Operand, operand2: &Operand) -> Result<Outcome, Self::Error>;

    fn float_neg(&mut self, destination: &Operand, operand: &Operand) -> Result<Outcome, Self::Error>;
    fn float_abs(&mut self, destination: &Operand, operand: &Operand) -> Result<Outcome, Self::Error>;
    fn float_sqrt(&mut self, destination: &Operand, operand: &Operand) -> Result<Outcome, Self::Error>;

    fn float_of_int(&mut self, destination: &Operand, operand: &Operand) -> Result<Outcome, Self::Error>;
    fn float_of_float(&mut self, destination: &Operand, operand: &Operand) -> Result<Outcome, Self::Error>;

    fn float_truncate(&mut self, destination: &Operand, operand: &Operand) -> Result<Outcome, Self::Error>;
    fn float_ceiling(&mut self, destination: &Operand, operand: &Operand) -> Result<Outcome, Self::Error>;
    fn float_floor(&mut self, destination: &Operand, operand: &Operand) -> Result<Outcome, Self::Error>;
    fn float_round(&mut self, destination: &Operand, operand: &Operand) -> Result<Outcome, Self::Error>;

    fn subpiece(&mut self, destination: &Operand, operand: &Operand, amount: &Operand) -> Result<Outcome, Self::Error>;
    fn pop_count(&mut self, destination: &Operand, operand: &Operand) -> Result<Outcome, Self::Error>;

    fn skip(&mut self) -> Result<Outcome, Self::Error> {
        Ok(Outcome::Branch(Branch::Next))
    }

    fn lift<A>(&mut self, address: A) -> Result<StepState, Self::Error>
        where A: IntoAddress;

    fn interpreter_space(&self) -> Arc<AddressSpace>;
}
