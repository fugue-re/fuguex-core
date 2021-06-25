use fugue::ir::{AddressValue, IntoAddress};
use fugue::ir::il::pcode::PCodeOp;

use crate::traits::Interpreter;
use crate::types::{Bound, BranchOutcome, Outcome, StepOutcome};

pub struct Machine<I: Interpreter> {
    interpreter: I,
}

impl<I> From<I> for Machine<I> where I: Interpreter {
    fn from(interpreter: I) -> Self {
        Self {
            interpreter,
        }
    }
}

impl<I> Machine<I> where I: Interpreter {
    #[inline(always)]
    pub fn new(interpreter: I) -> Self {
        Self::from(interpreter)
    }

    pub fn step<A>(&mut self, address: A) -> Result<StepOutcome, I::Error>
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
                PCodeOp::Branch { ref destination } => {
                    self.interpreter.branch(destination)
                },
                PCodeOp::CBranch { ref destination, ref condition } => {
                    self.interpreter.cbranch(destination, condition)
                },
                PCodeOp::IBranch { ref destination } => {
                    self.interpreter.ibranch(destination)
                },
                PCodeOp::Call { ref destination } => {
                    self.interpreter.call(destination)
                },
                PCodeOp::ICall { ref destination } => {
                    self.interpreter.icall(destination)
                },
                PCodeOp::Intrinsic { name, ref operands, ref result } => {
                    self.interpreter.intrinsic(name, operands.as_ref(), result.as_ref())
                },
                PCodeOp::Return { ref destination } => {
                    self.interpreter.return_(destination)
                },

                PCodeOp::IntEq { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.int_eq(result, operand1, operand2)
                },
                PCodeOp::IntNotEq { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.int_not_eq(result, operand1, operand2)
                },
                PCodeOp::IntLess { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.int_less(result, operand1, operand2)
                },
                PCodeOp::IntLessEq { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.int_less_eq(result, operand1, operand2)
                },
                PCodeOp::IntSLess { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.int_sless(result, operand1, operand2)
                },
                PCodeOp::IntSLessEq { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.int_sless_eq(result, operand1, operand2)
                },

                PCodeOp::IntZExt { ref result, ref operand } => {
                    self.interpreter.int_zext(result, operand)
                },
                PCodeOp::IntSExt { ref result, ref operand } => {
                    self.interpreter.int_sext(result, operand)
                },

                PCodeOp::IntAdd { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.int_add(result, operand1, operand2)
                },
                PCodeOp::IntSub { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.int_sub(result, operand1, operand2)
                },
                PCodeOp::IntCarry { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.int_carry(result, operand1, operand2)
                },
                PCodeOp::IntSCarry { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.int_scarry(result, operand1, operand2)
                },
                PCodeOp::IntSBorrow { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.int_sborrow(result, operand1, operand2)
                },

                PCodeOp::IntNeg { ref result, ref operand } => {
                    self.interpreter.int_neg(result, operand)
                },
                PCodeOp::IntNot { ref result, ref operand } => {
                    self.interpreter.int_not(result, operand)
                },

                PCodeOp::IntXor { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.int_xor(result, operand1, operand2)
                },
                PCodeOp::IntAnd { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.int_and(result, operand1, operand2)
                },
                PCodeOp::IntOr { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.int_or(result, operand1, operand2)
                },
                PCodeOp::IntLeftShift { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.int_left_shift(result, operand1, operand2)
                },
                PCodeOp::IntRightShift { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.int_right_shift(result, operand1, operand2)
                },
                PCodeOp::IntSRightShift { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.int_sright_shift(result, operand1, operand2)
                },

                PCodeOp::IntMul { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.int_mul(result, operand1, operand2)
                },
                PCodeOp::IntDiv { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.int_div(result, operand1, operand2)
                },
                PCodeOp::IntSDiv { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.int_sdiv(result, operand1, operand2)
                },
                PCodeOp::IntRem { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.int_rem(result, operand1, operand2)
                },
                PCodeOp::IntSRem { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.int_srem(result, operand1, operand2)
                },

                PCodeOp::BoolNot { ref result, ref operand } => {
                    self.interpreter.bool_not(result, operand)
                },
                PCodeOp::BoolXor { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.bool_xor(result, operand1, operand2)
                },
                PCodeOp::BoolAnd { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.bool_and(result, operand1, operand2)
                },
                PCodeOp::BoolOr { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.bool_or(result, operand1, operand2)
                },

                PCodeOp::FloatEq { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.float_eq(result, operand1, operand2)
                },
                PCodeOp::FloatNotEq { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.float_not_eq(result, operand1, operand2)
                },
                PCodeOp::FloatLess { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.float_less(result, operand1, operand2)
                },
                PCodeOp::FloatLessEq { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.float_less_eq(result, operand1, operand2)
                },

                PCodeOp::FloatIsNaN { ref result, ref operand } => {
                    self.interpreter.float_is_nan(result, operand)
                },

                PCodeOp::FloatAdd { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.float_add(result, operand1, operand2)
                },
                PCodeOp::FloatDiv { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.float_div(result, operand1, operand2)
                },
                PCodeOp::FloatMul { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.float_mul(result, operand1, operand2)
                },
                PCodeOp::FloatSub { ref result, operands: [ref operand1, ref operand2] } => {
                    self.interpreter.float_sub(result, operand1, operand2)
                },

                PCodeOp::FloatNeg { ref result, ref operand } => {
                    self.interpreter.float_neg(result, operand)
                },
                PCodeOp::FloatAbs { ref result, ref operand } => {
                    self.interpreter.float_abs(result, operand)
                },
                PCodeOp::FloatSqrt { ref result, ref operand } => {
                    self.interpreter.float_sqrt(result, operand)
                },

                PCodeOp::FloatOfInt { ref result, ref operand } => {
                    self.interpreter.float_of_int(result, operand)
                },
                PCodeOp::FloatOfFloat { ref result, ref operand } => {
                    self.interpreter.float_of_float(result, operand)
                },

                PCodeOp::FloatTruncate { ref result, ref operand } => {
                    self.interpreter.float_truncate(result, operand)
                },
                PCodeOp::FloatCeiling { ref result, ref operand } => {
                    self.interpreter.float_ceiling(result, operand)
                },
                PCodeOp::FloatFloor { ref result, ref operand } => {
                    self.interpreter.float_floor(result, operand)
                },
                PCodeOp::FloatRound { ref result, ref operand } => {
                    self.interpreter.float_round(result, operand)
                },

                PCodeOp::Subpiece { ref result, ref operand, ref amount } => {
                    self.interpreter.subpiece(result, operand, amount)
                },
                PCodeOp::PopCount { ref result, ref operand } => {
                    self.interpreter.pop_count(result, operand)
                },

                PCodeOp::Skip => {
                    self.interpreter.skip()
                },
            }?;

            match action {
                Outcome::Halt => {
                    return Ok(StepOutcome::Halt)
                },
                Outcome::Branch(ref branch) => if let BranchOutcome::Global(address) = step_state.branch(branch) {
                    return Ok(StepOutcome::Branch(address))
                } else {
                    continue
                },
            }
        }

        Ok(StepOutcome::Branch(step_state.fallthrough()))
    }

    pub fn step_until<A, B>(&mut self, address: A, until: Bound<B>) -> Result<Bound<AddressValue>, I::Error>
        where A: IntoAddress,
              B: IntoAddress {
        let space = self.interpreter.interpreter_space();
        let mut bound = until.in_space(space.clone());
        let mut address = address.into_address_value(space);

        while !bound.reached(&address) {
            bound = bound.deplete();
            if let StepOutcome::Branch(next_address) = self.step(&address)? {
                address = next_address;
            } else {
                break
            }
        }

        Ok(bound)
    }

    pub fn interpreter(&self) -> &I {
        &self.interpreter
    }

    pub fn interpreter_mut(&mut self) -> &mut I {
        &mut self.interpreter
    }
}
