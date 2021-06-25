use fugue::ir::Address;
use fugue::ir::il::pcode::{Operand, Register};
use fuguex_state::State;

pub trait Hook<'space> {
    type State: State + 'space;
    type Error: std::error::Error + Send + Sync + 'space;
}

pub trait HookMemoryRead<'space>: Hook<'space> {
    fn hook_memory_read(
        &mut self,
        state: &mut Self::State,
        address: &Address<'space>,
        value: &[<Self::State as State>::Value]
    ) -> Result<(), Self::Error>;
}

pub trait HookMemoryWrite<'space>: Hook<'space> {
    fn hook_memory_write(
        &mut self,
        state: &mut Self::State,
        address: &Address<'space>,
        value: &mut [<Self::State as State>::Value]
    ) -> Result<(), Self::Error>;
}

pub trait HookRegisterRead<'space>: Hook<'space> {
    fn hook_register_read(
        &mut self,
        state: &mut Self::State,
        register: &Register<'space>,
        value: &[<Self::State as State>::Value]
    ) -> Result<(), Self::Error>;
}

pub trait HookRegisterWrite<'space>: Hook<'space> {
    fn hook_register_write(
        &mut self,
        state: &mut Self::State,
        register: &Register<'space>,
        value: &mut [<Self::State as State>::Value]
    ) -> Result<(), Self::Error>;
}

pub trait HookCall<'space>: Hook<'space> {
    fn hook_call(
        &mut self,
        state: &mut Self::State,
        destination: &Address<'space>,
    ) -> Result<(), Self::Error>;
}

pub trait HookBranch<'space>: Hook<'space> {
    fn hook_branch(
        &mut self,
        state: &mut Self::State,
        destination: &Address<'space>,
    ) -> Result<(), Self::Error>;
}

pub trait HookCBranch<'space>: Hook<'space> {
    fn hook_cbranch(
        &mut self,
        state: &mut Self::State,
        destination: &Address<'space>,
        condition: &Operand<'space>,
    ) -> Result<(), Self::Error>;
}
