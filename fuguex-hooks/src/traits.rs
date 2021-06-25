use fugue::ir::Address;
use fugue::ir::il::pcode::{Operand, Register};

use fuguex_state::State;

use crate::types::{HookAction, HookBranchAction, HookCBranchAction, HookCallAction};

pub trait Hook {
    type State: State;
    type Error: std::error::Error + Send + Sync;
}

pub trait HookMemoryRead: Hook {
    fn hook_memory_read(
        &mut self,
        state: &mut Self::State,
        address: &Address,
        value: &[<Self::State as State>::Value]
    ) -> Result<HookAction, Self::Error>;
}

pub trait HookMemoryWrite: Hook {
    fn hook_memory_write(
        &mut self,
        state: &mut Self::State,
        address: &Address,
        value: &mut [<Self::State as State>::Value]
    ) -> Result<HookAction, Self::Error>;
}

pub trait HookRegisterRead: Hook {
    fn hook_register_read(
        &mut self,
        state: &mut Self::State,
        register: &Register,
        value: &[<Self::State as State>::Value]
    ) -> Result<HookAction, Self::Error>;
}

pub trait HookRegisterWrite: Hook {
    fn hook_register_write(
        &mut self,
        state: &mut Self::State,
        register: &Register,
        value: &mut [<Self::State as State>::Value]
    ) -> Result<HookAction, Self::Error>;
}

pub trait HookCall: Hook {
    fn hook_call(
        &mut self,
        state: &mut Self::State,
        destination: &Address,
    ) -> Result<HookCallAction, Self::Error>;
}

pub trait HookBranch: Hook {
    fn hook_branch(
        &mut self,
        state: &mut Self::State,
        destination: &Address,
    ) -> Result<HookBranchAction, Self::Error>;
}

pub trait HookCBranch: Hook {
    fn hook_cbranch(
        &mut self,
        state: &mut Self::State,
        destination: &Address,
        condition: &Operand,
    ) -> Result<HookCBranchAction, Self::Error>;
}
