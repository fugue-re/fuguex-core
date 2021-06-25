use fugue::ir::{Address, IntoAddress};
use interval_tree::{IntervalTree, Interval, Entry};

use std::iter::FromIterator;
use std::ops::Range;
use std::mem::take;

use thiserror::Error;

use crate::chunked::{self, ChunkState};
use crate::flat::{self, FlatState};
use crate::traits::{State, StateValue};

#[derive(Debug, Error)]
pub enum Error {
    #[error("unmapped virtual address at {address}")]
    UnmappedAddress { address: Address, size: usize },
    #[error("overlapped access from {address} byte access at {size}")]
    OverlappedAccess { address: Address, size: usize },
    #[error("overlapped mapping of {size} bytes from {address}")]
    OverlappedMapping { address: Address, size: usize },
    #[error(transparent)]
    Backing(flat::Error),
    #[error(transparent)]
    Chunked(chunked::Error),
}

impl Error {
    fn backing(base: Address, e: flat::Error) -> Self {
        Self::Backing(match e {
            flat::Error::OOBRead { address, size } => flat::Error::OOBRead {
                address: address + base,
                size,
            },
            flat::Error::OOBWrite { address, size } => flat::Error::OOBWrite {
                address: address + base,
                size,
            },
            flat::Error::AccessViolation { address, access, size } => flat::Error::AccessViolation {
                address: address + base,
                access,
                size,
            },
        })
    }
}

#[derive(Debug, Clone)]
pub enum Segment<T: StateValue> {
    Static { name: String, offset: usize },
    Mapping { name: String, backing: ChunkState<T> },
}

impl<T: StateValue> Segment<T> {
    pub fn new<S: AsRef<str>>(name: S, offset: usize) -> Self {
        Self::Static {
            name: name.as_ref().to_string(),
            offset,
        }
    }

    pub fn mapping<S: AsRef<str>>(name: S, mapping: ChunkState<T>) -> Self {
        Self::Mapping {
            name: name.as_ref().to_string(),
            backing: mapping,
        }
    }

    pub fn is_static(&self) -> bool {
        matches!(self, Self::Static { .. })
    }

    pub fn is_mapping(&self) -> bool {
        matches!(self, Self::Mapping { .. })
    }

    pub fn as_mapping(&self) -> Option<&ChunkState<T>> {
        if let Self::Mapping { ref backing, .. } = self {
            Some(backing)
        } else {
            None
        }
    }

    pub fn as_mapping_mut(&mut self) -> Option<&mut ChunkState<T>> {
        if let Self::Mapping { ref mut backing, .. } = self {
            Some(backing)
        } else {
            None
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Self::Static { name, .. } | Self::Mapping { name, .. } => name,
        }
    }

    pub fn fork(&self) -> Self {
        match self {
            Self::Static { .. } => self.clone(),
            Self::Mapping { name, backing } => Self::Mapping {
                name: name.clone(),
                backing: backing.fork(),
            },
        }
    }

    pub fn restore(&mut self, other: &Self) {
        match (self, other) {
            (Self::Static { name, offset }, Self::Static { name: rname, offset: roffset }) => {
                if name != rname || offset != roffset {
                    panic!("attempting to restore segment `{}` at {} from incompatible segment `{}` at {}",
                           name,
                           offset,
                           rname,
                           roffset
                    );
                }
            },
            (Self::Mapping { name, backing }, Self::Mapping { name: rname, backing: rbacking }) => {
                if name != rname ||
                    backing.base_address() != rbacking.base_address() ||
                    backing.len() != rbacking.len() {
                    panic!("attempting to restore segment `{}` at {} of size {} from incompatible segment `{}` at {} of size {}",
                           name,
                           backing.base_address(),
                           backing.len(),
                           rname,
                           rbacking.base_address(),
                           rbacking.len(),
                    );
                }

                backing.restore(rbacking);
            },
            (slf, oth) => panic!("attempting to restore segment `{}` from segment `{}` which have different kinds",
                                 slf.name(),
                                 oth.name()
            ),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PagedState<T: StateValue> {
    segments: IntervalTree<Address, Segment<T>>,
    inner: FlatState<T>,
}

impl<T: StateValue> AsRef<Self> for PagedState<T> {
    #[inline(always)]
    fn as_ref(&self) -> &Self {
        self
    }
}

impl<T: StateValue> AsMut<Self> for PagedState<T> {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut Self {
        self
    }
}

impl<T: StateValue> PagedState<T> {
    pub fn from_parts(
        mapping: impl IntoIterator<Item = (Range<Address>, Segment<T>)>,
        backing: FlatState<T>,
    ) -> Self {
        Self {
            segments: IntervalTree::from_iter(mapping.into_iter().map(|(r, s)| {
                (Interval::from(r.start..=(r.end - 1usize)), s)
            })),
            inner: backing,
        }
    }

    pub fn mapping<S, A>(&mut self, name: S, base_address: A, size: usize) -> Result<(), Error>
    where S: AsRef<str>,
          A: IntoAddress {
        let base_address = base_address.into_address(self.inner.address_space().as_ref());
        let range = base_address..=(base_address + size - 1usize); // TODO: error for zero-size

        if self.segments.overlaps(range.clone()) {
            return Err(Error::OverlappedMapping {
                address: base_address,
                size,
            })
        }

        self.segments.insert(range, Segment::mapping(name, ChunkState::new(self.inner.address_space(), base_address, size)));
        Ok(())
    }

    pub fn segments(&self) -> &IntervalTree<Address, Segment<T>> {
        &self.segments
    }

    pub fn mappings(&self) -> impl Iterator<Item=&ChunkState<T>> {
        self.segments.values().filter_map(|v| if let Segment::Mapping { backing, .. } = v {
            Some(backing)
        } else {
            None
        })
    }

    pub fn mapping_for<A>(&self, address: A) -> Option<&ChunkState<T>>
    where A: IntoAddress {
        let address = address.into_address(self.inner.address_space().as_ref());
        self.segments.find(address)
            .and_then(|e| e.value().as_mapping())
    }

    pub fn mapping_for_mut<A>(&mut self, address: A) -> Option<&mut ChunkState<T>>
    where A: IntoAddress {
        let address = address.into_address(self.inner.address_space().as_ref());
        self.segments.find_mut(address)
            .and_then(|e| e.into_value().as_mapping_mut())
    }

    pub fn inner(&self) -> &FlatState<T> {
        &self.inner
    }

    pub fn inner_mut(&mut self) -> &mut FlatState<T> {
        &mut self.inner
    }
}

impl<T: StateValue> PagedState<T> {
    pub fn with_flat<'a, A, F, O: 'a>(&'a self, address: A, access_size: usize, f: F) -> Result<O, Error>
    where A: IntoAddress,
          F: FnOnce(&'a FlatState<T>, Address, usize) -> Result<O, Error> {
        let address = address.into_address(self.inner.address_space().as_ref());
        if let Some(principal) = self.segments.find(&address) {
            if address + access_size - 1usize > *principal.interval().end() { // FIXME: checked arith.
                return Err(Error::OverlappedAccess {
                    address,
                    size: access_size,
                });
            }

            match principal.value() {
                Segment::Mapping { ref backing, .. } => {
                    let translated = backing.translate_checked(address, access_size)
                        .map_err(Error::Chunked)?;
                    f(backing.inner(), translated.into_address(self.inner.address_space().as_ref()), access_size)
                },
                Segment::Static { offset, .. } => {
                    let address = (address - *principal.interval().start()) + *offset;
                    f(&self.inner, address.into_address(self.inner.address_space().as_ref()), access_size)
                },
            }
        } else {
            Err(Error::UnmappedAddress { address, size: access_size })
        }
    }

    pub fn with_flat_mut<'a, A, F, O: 'a>(&'a mut self, address: A, access_size: usize, f: F) -> Result<O, Error>
    where A: IntoAddress,
          F: FnOnce(&'a mut FlatState<T>, Address, usize) -> Result<O, Error> {
        let space = self.inner.address_space();
        let address = address.into_address(space.as_ref());
        if let Some(principal) = self.segments.find_mut(&address) {
            let interval = principal.interval();
            if address + access_size - 1usize > *interval.end() {
                return Err(Error::OverlappedAccess {
                    address,
                    size: access_size,
                });
            }
            match principal.into_value() {
                Segment::Mapping { ref mut backing, .. } => {
                    let translated = backing.translate_checked(address, access_size)
                        .map_err(Error::Chunked)?;
                    f(backing.inner_mut(), translated.into_address(space.as_ref()), access_size)
                },
                Segment::Static { offset, .. } => {
                    let address = (address - *interval.start()) + *offset;
                    f(&mut self.inner, address.into_address(space.as_ref()), access_size)
                }
            }
        } else {
            Err(Error::UnmappedAddress { address, size: access_size })
        }
    }

    pub fn segment_bounds<A>(&self, address: A) -> Result<Entry<Address, Segment<T>>, Error>
    where A: IntoAddress {
        let address = address.into_address(self.inner.address_space().as_ref());
        self.segments
            .find(&address)
            .ok_or_else(|| Error::UnmappedAddress { address, size: 1usize })
    }
}

impl<V: StateValue> State for PagedState<V> {
    type Error = Error;
    type Value = V;

    fn fork(&self) -> Self {
        Self {
            segments: self.segments.iter().map(|(i, v)| (i, v.fork())).collect(),
            inner: self.inner.fork(),
        }
    }

    fn restore(&mut self, other: &Self) {
        self.inner.restore(&other.inner);

        let segments = take(&mut self.segments);
        self.segments = segments.into_iter()
            .filter_map(|(i, mut v)| if let Some(vo) = other.segments.find_exact(&i) {
                v.restore(vo.value());
                Some((i, v))
            } else {
                None
            })
            .collect();
    }

    fn copy_values<F, T>(&mut self, from: F, to: T, size: usize) -> Result<(), Error>
    where
        F: IntoAddress,
        T: IntoAddress,
    {
        let from = from.into_address(self.inner.address_space().as_ref());
        let to = to.into_address(self.inner.address_space().as_ref());

        // TODO: can we avoid the intermediate allocation?

        let vals = self.view_values(from, size)?.to_vec();
        let view = self.view_values_mut(to, size)?;

        for (d, s) in view.iter_mut().zip(vals.into_iter()) {
            *d = s;
        }

        Ok(())
    }

    fn get_values<A>(&self, address: A, values: &mut [Self::Value]) -> Result<(), Error>
    where
        A: IntoAddress,
    {
        let address = address.into_address(self.inner.address_space().as_ref());
        let n = values.len();

        self.with_flat(address, n, |inner, address, _size| {
            inner.get_values(address, values).map_err(|e| Error::backing(address, e))
        })
    }

    fn view_values<A>(&self, address: A, n: usize) -> Result<&[Self::Value], Error>
    where
        A: IntoAddress,
    {
        let address = address.into_address(self.inner.address_space().as_ref());
        self.with_flat(address, n, |inner, address, n| {
            inner.view_values(address, n).map_err(|e| Error::backing(address, e))
        })
    }

    fn view_values_mut<A>(&mut self, address: A, n: usize) -> Result<&mut [Self::Value], Error>
    where
        A: IntoAddress,
    {
        let address = address.into_address(self.inner.address_space().as_ref());
        self.with_flat_mut(address, n, |inner, address, n| {
            inner.view_values_mut(address, n).map_err(|e| Error::backing(address, e))
        })
    }

    fn set_values<A>(&mut self, address: A, values: &[Self::Value]) -> Result<(), Error>
    where
        A: IntoAddress,
    {
        let address = address.into_address(self.inner.address_space().as_ref());
        let n = values.len();
        self.with_flat_mut(address, n, |inner, address, _size| {
            inner.set_values(address, values).map_err(|e| Error::backing(address, e))
        })
    }

    fn len(&self) -> usize {
        // what to do here? sum of all sizes?
        self.inner.len() + self.mappings().map(|m| m.len()).sum::<usize>()
    }
}
