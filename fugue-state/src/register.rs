use std::ops::{Deref, DerefMut};

use fugue_core::ir::Translator;

use crate::flat::FlatState;

pub use crate::flat::Error;

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct RegisterState<'space>(FlatState<'space>);

impl<'space> Deref for RegisterState<'space> {
    type Target = FlatState<'space>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'space> DerefMut for RegisterState<'space> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<'space> From<RegisterState<'space>> for FlatState<'space> {
    fn from(t: RegisterState<'space>) -> Self {
        t.0
    }
}

impl<'space> RegisterState<'space> {
    pub fn new(translator: &'space Translator) -> Self {
        let space = translator.manager().register_space();
        let size = translator.register_space_size();
        Self(FlatState::new(space, size))
    }
}
