use std::ops::{Deref, DerefMut};

use fugue_core::ir::Translator;

use crate::flat::FlatState;

pub use crate::flat::Error;

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct UniqueState<'space>(FlatState<'space>);

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
