use fugue::ir::Address;
use fugue::ir::il::pcode::{Operand, Register};

use fuguex_state::State;

use crate::types::{Error, HookAction, HookCBranchAction, HookCallAction};

pub trait Hook {
    type State: State;
    type Error: std::error::Error + Send + Sync + 'static;
}

pub trait HookMemoryRead: Hook {
    fn hook_memory_read(
        &mut self,
        state: &mut Self::State,
        address: &Address,
        value: &[<Self::State as State>::Value]
    ) -> Result<HookAction, Error<Self::Error>>;
}

pub trait HookMemoryWrite: Hook {
    fn hook_memory_write(
        &mut self,
        state: &mut Self::State,
        address: &Address,
        value: &[<Self::State as State>::Value]
    ) -> Result<HookAction, Error<Self::Error>>;
}

pub trait HookRegisterRead: Hook {
    fn hook_register_read(
        &mut self,
        state: &mut Self::State,
        register: &Register,
        value: &[<Self::State as State>::Value]
    ) -> Result<HookAction, Error<Self::Error>>;
}

pub trait HookRegisterWrite: Hook {
    fn hook_register_write(
        &mut self,
        state: &mut Self::State,
        register: &Register,
        value: &[<Self::State as State>::Value]
    ) -> Result<HookAction, Error<Self::Error>>;
}

pub trait HookCall: Hook {
    fn hook_call(
        &mut self,
        state: &mut Self::State,
        destination: &Address,
    ) -> Result<HookCallAction, Error<Self::Error>>;
}

pub trait HookCBranch: Hook {
    fn hook_cbranch(
        &mut self,
        state: &mut Self::State,
        destination: &Operand,
        condition: &Operand,
    ) -> Result<HookCBranchAction, Error<Self::Error>>;
}
