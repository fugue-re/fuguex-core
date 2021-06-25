use std::marker::PhantomData;

use fugue::ir::{Address, AddressSpace, IntoAddress};
use fugue::ir::il::pcode::{PCode, PCodeOp};

pub enum Bound<'space, A: IntoAddress + 'space> {
    Address(A),
    Steps(usize, PhantomData<&'space ()>),
    Unbounded(PhantomData<&'space ()>),
}

impl<'space, A> Bound<'space, A> where A: IntoAddress + 'space {
    pub fn address(address: A) -> Bound<'space, A> {
        Self::Address(address)
    }

    pub fn in_space(self, space: &'space AddressSpace) -> Bound<'space, Address<'space>> {
        match self {
            Self::Address(address) => Bound::Address(address.into_address(space)),
            Self::Steps(steps, _) => Bound::Steps(steps, PhantomData),
            Self::Unbounded(_) => Bound::Unbounded(PhantomData),
        }
    }
}

impl<'space> Bound<'space, Address<'space>> {
    pub fn steps(steps: usize) -> Bound<'space, Address<'space>> {
        Self::Steps(steps, PhantomData)
    }

    pub fn unbounded() -> Bound<'space, Address<'space>> {
        Self::Unbounded(PhantomData)
    }

    pub fn deplete(self) -> Self {
        if let Self::Steps(steps, m) = self {
            Self::Steps(steps.checked_sub(1).unwrap_or(0), m)
        } else {
            self
        }
    }

    pub fn reached<A>(&self, address: A) -> bool
    where A: IntoAddress + 'space {
        match self {
            Self::Address(ref target) => *target == address.into_address(target.space()),
            Self::Steps(steps, _) => *steps == 0,
            Self::Unbounded(_) => false,
        }
    }
}

pub enum Branch<'space> {
    Next,
    Local(isize),
    Global(Address<'space>),
}

pub enum Outcome<'space> {
    Halt,
    Branch(Branch<'space>),
}

pub enum StepOutcome<'space> {
    Halt,
    Branch(Address<'space>),
}

pub struct StepState<'space> {
    pcode: PCode<'space>,
    position: usize,
}

pub enum BranchOutcome<'space> {
    Local,
    Global(Address<'space>),
}

impl<'space> From<PCode<'space>> for StepState<'space> {
    fn from(pcode: PCode<'space>) -> Self {
        Self {
            pcode,
            position: 0,
        }
    }
}

impl<'space> StepState<'space> {
    #[inline(always)]
    pub fn address(&self) -> Address<'space> {
        self.pcode.address()
    }

    #[inline(always)]
    pub fn current(&self) -> Option<&PCodeOp<'space>> {
        self.pcode.operations().get(self.position)
    }

    pub fn fallthrough(&self) -> Address<'space> {
        self.address() + self.pcode.length()
    }

    pub fn branch(&mut self, action: &Branch<'space>) -> BranchOutcome<'space> {
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
                return BranchOutcome::Global(*address)
            }
        }

        if self.position < self.pcode.operations.len() {
            BranchOutcome::Local
        } else {
            BranchOutcome::Global(self.fallthrough())
        }
    }
}
