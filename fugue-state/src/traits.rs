use fugue::ir::address::IntoAddress;

pub use fugue_state_derive::AsState;

pub trait State: Clone + Send + Sync {
    type Error: std::error::Error + Send + Sync;
    type Value: Clone + Send + Sync;

    fn fork(&self) -> Self;
    fn restore(&mut self, other: &Self);

    fn len(&self) -> usize;

    fn copy_values<F, T>(&mut self, from: F, to: T, size: usize) -> Result<(), Self::Error>
    where F: IntoAddress,
          T: IntoAddress;

    fn get_values<A>(&self, address: A, bytes: &mut [Self::Value]) -> Result<(), Self::Error>
    where A: IntoAddress;

    fn view_values<A>(&self, address: A, size: usize) -> Result<&[Self::Value], Self::Error>
    where A: IntoAddress;

    fn view_values_mut<A>(&mut self, address: A, size: usize) -> Result<&mut [Self::Value], Self::Error>
    where A: IntoAddress;

    fn set_values<A>(&mut self, address: A, bytes: &[Self::Value]) -> Result<(), Self::Error>
    where A: IntoAddress;
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
