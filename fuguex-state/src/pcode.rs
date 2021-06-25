use std::marker::PhantomData;

use fugue::bytes::Order;

use fugue::ir::il::pcode::Operand;
use fugue::ir::{IntoAddress, Translator};

use thiserror::Error;

use crate::paged::{self, PagedState};
use crate::register::{self, RegisterState};
use crate::unique::{self, UniqueState};

use crate::traits::{State, StateValue};
use crate::traits::{FromStateValues, IntoStateValues};

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Memory(paged::Error),
    #[error(transparent)]
    Register(register::Error),
    #[error(transparent)]
    Temporary(unique::Error),
}

#[derive(Debug, Clone)]
pub struct PCodeState<T: StateValue, O: Order> {
    memory: PagedState<T>,
    registers: RegisterState<T>,
    temporaries: UniqueState<T>,
    marker: PhantomData<O>,
}

impl<T: StateValue, O: Order> AsRef<Self> for PCodeState<T, O> {
    fn as_ref(&self) -> &Self {
        self
    }
}

impl<T: StateValue, O: Order> AsMut<Self> for PCodeState<T, O> {
    fn as_mut(&mut self) -> &mut Self {
        self
    }
}

impl<T: StateValue, O: Order> PCodeState<T, O> {
    pub fn new(memory: PagedState<T>, translator: &Translator) -> Self {
        Self {
            memory,
            registers: RegisterState::new(translator),
            temporaries: UniqueState::new(translator),
            marker: PhantomData,
        }
    }

    pub fn memory(&self) -> &PagedState<T> {
        &self.memory
    }

    pub fn memory_mut(&mut self) -> &mut PagedState<T> {
        &mut self.memory
    }

    pub fn registers(&self) -> &RegisterState<T> {
        &self.registers
    }

    pub fn registers_mut(&mut self) -> &mut RegisterState<T> {
        &mut self.registers
    }

    pub fn temporaries(&self) -> &UniqueState<T> {
        &self.temporaries
    }

    pub fn temporaries_mut(&mut self) -> &mut UniqueState<T> {
        &mut self.temporaries
    }

    pub fn with_operand_values<U, F>(&self, operand: &Operand, f: F) -> Result<U, Error>
    where F: FnOnce(&[T]) -> U {
        match operand {
            Operand::Address { value, size } => {
                self.memory()
                    .view_values(value.offset(), *size)
                    .map_err(Error::Memory)
                    .map(f)
            },
            Operand::Constant { value, size, .. } => {
                // max size of value
                let mut values: [T; 8] = Default::default();

                if O::ENDIAN.is_big() {
                    for (d, s) in values[..*size].iter_mut().zip(&value.to_be_bytes()[8-*size..]) {
                        *d = T::from_byte(*s);
                    }
                } else {
                    for (d, s) in values[..*size].iter_mut().zip(&value.to_le_bytes()[..*size]) {
                        *d = T::from_byte(*s);
                    }
                }

                Ok(f(&values[..*size]))
            },
            Operand::Register { offset, size, .. } => {
                self.registers()
                    .view_values(*offset, *size)
                    .map_err(Error::Register)
                    .map(f)
            },
            Operand::Variable { offset, size, .. } => {
                self.temporaries()
                    .view_values(*offset, *size)
                    .map_err(Error::Temporary)
                    .map(f)
            },
        }
    }

    pub fn with_operand_values_mut<U, F>(&mut self, operand: &Operand, f: F) -> Result<U, Error>
    where F: FnOnce(&mut [T]) -> U {
        match operand {
            Operand::Address { value, size } => {
                self.memory_mut()
                    .view_values_mut(value.offset(), *size)
                    .map_err(Error::Memory)
                    .map(f)
            },
            Operand::Register { offset, size, .. } => {
                self.registers_mut()
                    .view_values_mut(*offset, *size)
                    .map_err(Error::Register)
                    .map(f)
            },
            Operand::Variable { offset, size, .. } => {
                self.temporaries_mut()
                    .view_values_mut(*offset, *size)
                    .map_err(Error::Temporary)
                    .map(f)
            },
            Operand::Constant { .. } => {
                panic!("cannot mutate Operand::Constant");
            },
        }
    }

    pub fn get_operand<V: FromStateValues<T>>(&self, operand: &Operand) -> Result<V, Error> {
        self.with_operand_values(operand, |values| V::from_values::<O>(values))
    }

    pub fn set_operand<V: IntoStateValues<T>>(&mut self, operand: &Operand, value: V) -> Result<(), Error> {
        self.with_operand_values_mut(operand, |values| value.into_values::<O>(values))
    }
}

impl<V: StateValue, O: Order> State for PCodeState<V, O> {
    type Error = Error;
    type Value = V;

    fn fork(&self) -> Self {
        Self {
            registers: self.registers.fork(),
            temporaries: self.temporaries.fork(),
            memory: self.memory.fork(),
            marker: self.marker,
        }
    }

    fn restore(&mut self, other: &Self) {
        self.registers.restore(&other.registers);
        self.temporaries.restore(&other.temporaries);
        self.memory.restore(&other.memory);
    }

    #[inline(always)]
    fn copy_values<F, T>(&mut self, from: F, to: T, size: usize) -> Result<(), Self::Error>
    where F: IntoAddress,
          T: IntoAddress {
        self.memory.copy_values(from, to, size)
            .map_err(Error::Memory)
    }

    #[inline(always)]
    fn get_values<A>(&self, address: A, values: &mut [Self::Value]) -> Result<(), Self::Error>
    where A: IntoAddress {
        self.memory.get_values(address, values)
            .map_err(Error::Memory)
    }

    #[inline(always)]
    fn view_values<A>(&self, address: A, size: usize) -> Result<&[Self::Value], Self::Error>
    where A: IntoAddress {
        self.memory.view_values(address, size)
            .map_err(Error::Memory)
    }

    #[inline(always)]
    fn view_values_mut<A>(&mut self, address: A, size: usize) -> Result<&mut [Self::Value], Self::Error>
    where A: IntoAddress {
        self.memory.view_values_mut(address, size)
            .map_err(Error::Memory)
    }

    #[inline(always)]
    fn set_values<A>(&mut self, address: A, values: &[Self::Value]) -> Result<(), Self::Error>
    where A: IntoAddress {
        self.memory.set_values(address, values)
            .map_err(Error::Memory)
    }

    #[inline(always)]
    fn len(&self) -> usize {
        self.memory.len()
    }
}
