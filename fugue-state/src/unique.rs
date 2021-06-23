use std::ops::{Deref, DerefMut};

use fugue::ir::{IntoAddress, Translator};

use crate::flat::FlatState;
use crate::traits::State;

pub use crate::flat::Error;

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct UniqueState<'space>(FlatState<'space>);

impl<'space> AsRef<Self> for UniqueState<'space> {
    #[inline(always)]
    fn as_ref(&self) -> &Self {
        self
    }
}

impl<'space> AsMut<Self> for UniqueState<'space> {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut Self {
        self
    }
}

impl<'space> AsRef<FlatState<'space>> for UniqueState<'space> {
    #[inline(always)]
    fn as_ref(&self) -> &FlatState<'space> {
        &self.0
    }
}

impl<'space> AsMut<FlatState<'space>> for UniqueState<'space> {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut FlatState<'space> {
        &mut self.0
    }
}

impl<'space> Deref for UniqueState<'space> {
    type Target = FlatState<'space>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'space> DerefMut for UniqueState<'space> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<'space> From<UniqueState<'space>> for FlatState<'space> {
    fn from(t: UniqueState<'space>) -> Self {
        t.0
    }
}

impl<'space> State for UniqueState<'space> {
    type Error = Error<'space>;
    type Value = u8;

    #[inline(always)]
    fn fork(&self) -> Self {
        Self(self.0.fork())
    }

    #[inline(always)]
    fn restore(&mut self, other: &Self) {
        self.0.restore(&other.0)
    }

    #[inline(always)]
    fn len(&self) -> usize {
        self.0.len()
    }

    #[inline(always)]
    fn copy_values<F, T>(&mut self, from: F, to: T, size: usize) -> Result<(), Self::Error>
    where F: IntoAddress,
          T: IntoAddress {
        self.0.copy_values(from, to, size)
    }

    #[inline(always)]
    fn get_values<A>(&self, address: A, bytes: &mut [Self::Value]) -> Result<(), Self::Error>
    where A: IntoAddress {
        self.0.get_values(address, bytes)
    }

    #[inline(always)]
    fn view_values<A>(&self, address: A, size: usize) -> Result<&[Self::Value], Self::Error>
    where A: IntoAddress {
        self.0.view_values(address, size)
    }

    #[inline(always)]
    fn view_values_mut<A>(&mut self, address: A, size: usize) -> Result<&mut [Self::Value], Self::Error>
    where A: IntoAddress {
        self.0.view_values_mut(address, size)
    }

    #[inline(always)]
    fn set_values<A>(&mut self, address: A, bytes: &[Self::Value]) -> Result<(), Self::Error>
    where A: IntoAddress {
        self.0.set_values(address, bytes)
    }
}

impl<'space> UniqueState<'space> {
    pub fn new(translator: &'space Translator) -> Self {
        let space = translator.manager().unique_space();
        let size = translator.unique_space_size();
        Self(FlatState::new(space, size))
    }
}
