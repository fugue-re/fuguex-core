use std::ops::{Deref, DerefMut};

use fugue::ir::Translator;

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

impl<'space> AsRef<FlatState<'space>> for UniqueState<'space> {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut FlatState<'space> {
        &mut self
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

impl<'space> UniqueState<'space> {
    pub fn new(translator: &'space Translator) -> Self {
        let space = translator.manager().unique_space();
        let size = translator.unique_space_size();
        Self(FlatState::new(space, size))
    }
}
