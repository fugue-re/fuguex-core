use fugue::ir::{AddressSpace, AddressValue, IntoAddress};
use fugue::ir::il::pcode::{PCode, PCodeOp};

use std::sync::Arc;

#[derive(Debug, Clone)]
pub enum Bound<A: IntoAddress> {
    Address(A),
    Steps(usize),
    Unbounded,
}

impl<A> Bound<A> where A: IntoAddress {
    pub fn address(address: A) -> Bound<A> {
        Self::Address(address)
    }

    pub fn in_space(self, space: Arc<AddressSpace>) -> Bound<AddressValue> {
        match self {
            Self::Address(address) => Bound::Address(address.into_address_value(space)),
            Self::Steps(steps) => Bound::Steps(steps),
            Self::Unbounded => Bound::Unbounded,
        }
    }
}

impl Bound<AddressValue> {
    pub fn steps(steps: usize) -> Bound<AddressValue> {
        Self::Steps(steps)
    }

    pub fn unbounded() -> Bound<AddressValue> {
        Self::Unbounded
    }

    pub fn deplete(self) -> Self {
        if let Self::Steps(steps) = self {
            Self::Steps(steps.checked_sub(1).unwrap_or(0))
        } else {
            self
        }
    }

    pub fn reached<A>(&self, address: A) -> bool
    where A: IntoAddress  {
        match self {
            Self::Address(ref target) => *target == address.into_address_value(target.space()),
            Self::Steps(steps) => *steps == 0,
            Self::Unbounded => false,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Branch {
    Next,
    Local(isize),
    Global(AddressValue),
}

#[derive(Debug, Clone)]
pub enum Outcome {
    Halt,
    Branch(Branch),
}

#[derive(Debug, Clone)]
pub enum StepOutcome {
    Halt,
    Branch(AddressValue),
}

#[derive(Debug, Clone)]
pub enum BranchOutcome {
    Local,
    Global(AddressValue),
}

#[derive(Debug, Clone)]
pub struct StepState {
    pcode: PCode,
    position: usize,
}

impl From<PCode> for StepState {
    fn from(pcode: PCode) -> Self {
        Self {
            pcode,
            position: 0,
        }
    }
}

impl StepState {
    #[inline(always)]
    pub fn address(&self) -> AddressValue {
        self.pcode.address()
    }

    #[inline(always)]
    pub fn current(&self) -> Option<&PCodeOp> {
        self.pcode.operations().get(self.position)
    }

    pub fn fallthrough(&self) -> AddressValue {
        self.address() + self.pcode.length()
    }

    pub fn branch(&mut self, action: &Branch) -> BranchOutcome {
        match action {
            Branch::Next => {
                self.position += 1;
            },
            Branch::Local(offset) => {
                if offset.is_negative() {
                    let abs = offset.wrapping_abs() as usize;
                    if abs <= self.position {
                        self.position -= abs;
                    } else {
                        panic!("negative local branch out of range")
                    }
                } else {
                    self.position += *offset as usize;
                }
            },
            Branch::Global(address) => {
                return BranchOutcome::Global(address.clone())
            }
        }

        if self.position < self.pcode.operations.len() {
            BranchOutcome::Local
        } else {
            BranchOutcome::Global(self.fallthrough())
        }
    }
}
