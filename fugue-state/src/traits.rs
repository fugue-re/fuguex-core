use fugue::ir::Address;
use fugue::ir::address::IntoAddress;

pub use fugue_state_derive::AsState;

pub trait State: Clone + Send + Sync + 'static {
    type Error: std::error::Error + Send + Sync + 'static;

    fn fork(&self) -> Self;
    fn restore(&mut self, other: &Self);

    fn copy_bytes<F, T>(&mut self, from: F, to: T, size: usize) -> Result<(), Self::Error>
    where F: IntoAddress,
          T: IntoAddress;

    fn get_bytes<A>(&self, address: A, bytes: &mut [u8]) -> Result<(), Self::Error>
    where A: IntoAddress;

    fn view_bytes<A>(&self, address: A, size: usize) -> Result<&[u8], Self::Error>
    where A: IntoAddress;

    fn view_bytes_mut<A>(&mut self, address: A, size: usize) -> Result<&mut [u8], Self::Error>
    where A: IntoAddress;

    fn set_bytes<A>(&mut self, address: A, bytes: &[u8]) -> Result<(), Self::Error>
    where A: IntoAddress;

    fn len(&self) -> usize;
}

pub trait AsState<S>: State {
    fn state_ref(&self) -> &S;
    fn state_mut(&mut self) -> &mut S;
}

impl<S, T> AsState<S> for T where T: State + AsRef<S> + AsMut<S> {
    fn state_ref(&self) -> &S {
        self.as_ref()
    }

    fn state_mut(&mut self) -> &mut S {
        self.as_mut()
    }
}

pub trait AsState2<S, T>: State + AsState<S> + AsState<T> {
    fn state2_ref(&self) -> (&S, &T) {
        (self.state_ref(), self.state_ref())
    }

    fn state2_mut(&mut self) -> (&mut S, &mut T);
}

pub trait AsState3<S, T, U>: State + AsState<S> + AsState<T> + AsState<U> {
    fn state3_ref(&self) -> (&S, &T, &U) {
        (self.state_ref(), self.state_ref(), self.state_ref())
    }

    fn state3_mut(&mut self) -> (&mut S, &mut T, &mut U);
}

pub trait AsState4<S, T, U, V>: State + AsState<S> + AsState<T> + AsState<U> + AsState<V> {
    fn state4_ref(&self) -> (&S, &T, &U, &V) {
        (self.state_ref(), self.state_ref(), self.state_ref(), self.state_ref())
    }

    fn state4_mut(&mut self) -> (&mut S, &mut T, &mut U, &mut V);
}
