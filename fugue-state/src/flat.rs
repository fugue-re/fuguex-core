use std::mem::size_of;

use fugue_core::ir::{Address, AddressSpace};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error<'space> {
    #[error("out-of-bounds read of `{size}` bytes at {address}")]
    OOBRead { address: Address<'space>, size: usize },
    #[error("out-of-bounds write of `{size}` bytes at {address}")]
    OOBWrite { address: Address<'space>, size: usize },
    #[error("address {address} of space `{}` cannot be dereferenced in `{}`", address.space().name(), space.name())]
    InvalidAddress { address: Address<'space>, space: &'space AddressSpace },
}

type Result<'space, T> = std::result::Result<T, Error<'space>>;

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct State<'space> {
    backing: Vec<u8>,
    dirty: DirtyBacking,
    space: &'space AddressSpace,
}

impl<'space> State<'space> {
    pub fn new(space: &'space AddressSpace, size: usize) -> Self {
        Self {
            backing: vec![0u8; size],
            dirty: DirtyBacking::new(size),
            space,
        }
    }

    pub fn from_vec(space: &'space AddressSpace, bytes: Vec<u8>) -> Self {
        let size = bytes.len();
        Self {
            backing: bytes,
            dirty: DirtyBacking::new(size),
            space,
        }
    }

    pub fn fork(&self) -> Self {
        Self {
            backing: self.backing.clone(),
            dirty: self.dirty.fork(),
            space: self.space,
        }
    }

    pub fn restore(&mut self, other: &Self) {
        for block in &self.dirty.indices {
            let start = block.start_address(self.space).offset() as usize;
            let end = block.end_address(self.space).offset() as usize;

            let real_end = self.backing.len().min(end);

            self.dirty.bitsmap[block.index()] = 0;
            self.backing[start..real_end].copy_from_slice(&other.backing[start..real_end]);
        }
    }

    pub fn copy_bytes(&mut self, from: &Address<'space>, to: &Address<'space>, size: usize) -> Result<()> {
        if self.space != from.space() {
            return Err(Error::InvalidAddress {
                address: from.clone(),
                space: self.space,
            })
        }

        if self.space != to.space() {
            return Err(Error::InvalidAddress {
                address: to.clone(),
                space: self.space,
            })
        }

        let soff = from.offset() as usize;
        let doff = to.offset() as usize;

        if soff > self.len() || soff.checked_add(size).is_none() || soff + size > self.len() {
            return Err(Error::OOBRead {
                address: from.clone(),
                size, //(soff + size) - self.len(),
            });
        }

        if doff > self.len() || doff.checked_add(size).is_none() || doff + size > self.len() {
            return Err(Error::OOBWrite {
                address: to.clone(),
                size, // (doff + size) - self.len(),
            });
        }

        self.backing.copy_within(soff..(soff + size), doff);

        /*
        unsafe {
            use std::ptr;

            let src_ptr = self.backing.as_ptr().offset(usize::from(from) as isize);
            let dst_ptr = self.backing.as_mut_ptr().offset(usize::from(to) as isize);

            ptr::copy(src_ptr, dst_ptr, size)
        }
        */

        self.dirty.dirty_region(to, size);

        Ok(())
    }

    pub fn get_bytes(&self, address: &Address<'space>, bytes: &mut [u8]) -> Result<()> {
        if self.space != address.space() {
            return Err(Error::InvalidAddress {
                address: address.clone(),
                space: self.space,
            })
        }

        let size = bytes.len();
        let start = address.offset() as usize;
        let end = start.checked_add(size);

        if start > self.len() || end.is_none() || end.unwrap() > self.len() {
            return Err(Error::OOBRead {
                address: address.clone(),
                size: bytes.len(),
            });
        }

        let end = end.unwrap();

        bytes[..].copy_from_slice(&self.backing[start..end]);

        Ok(())
    }

    pub fn view_bytes(&self, address: &Address<'space>, size: usize) -> Result<&[u8]> {
        if self.space != address.space() {
            return Err(Error::InvalidAddress {
                address: address.clone(),
                space: self.space,
            })
        }

        let start = address.offset() as usize;
        let end = start.checked_add(size);

        if start > self.len() || end.is_none() || end.unwrap() > self.len() {
            return Err(Error::OOBRead {
                address: address.clone(),
                size,
            });
        }

        let end = end.unwrap();

        Ok(&self.backing[start..end])
    }

    pub fn view_bytes_mut(&mut self, address: &Address<'space>, size: usize) -> Result<&mut [u8]> {
        if self.space != address.space() {
            return Err(Error::InvalidAddress {
                address: address.clone(),
                space: self.space,
            })
        }

        let start = address.offset() as usize;
        let end = start.checked_add(size);

        if start > self.len() || end.is_none() || end.unwrap() > self.len() {
            return Err(Error::OOBRead {
                address: address.clone(),
                size,
            });
        }

        let end = end.unwrap();

        self.dirty.dirty_region(address, size);

        Ok(&mut self.backing[start..end])
    }

    pub fn set_bytes<A>(&mut self, address: &Address<'space>, bytes: &[u8]) -> Result<()> {
        if self.space != address.space() {
            return Err(Error::InvalidAddress {
                address: address.clone(),
                space: self.space,
            })
        }

        let size = bytes.len();
        let start = address.offset() as usize;
        let end = start.checked_add(size);

        if start > self.len() || end.is_none() || end.unwrap() > self.len() {
            return Err(Error::OOBWrite {
                address: address.clone(),
                size,
            });
        }

        let end = end.unwrap();

        self.backing[start..end].copy_from_slice(bytes);
        self.dirty.dirty_region(address, size);

        Ok(())
    }

    pub fn len(&self) -> usize {
        self.backing.len()
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[repr(transparent)]
struct Block(u64);

pub const BLOCK_SIZE: u64 = 64;

impl<'space> From<&'_ Address<'space>> for Block {
    fn from(t: &Address<'space>) -> Block {
        Self(t.offset() / BLOCK_SIZE)
    }
}

impl<'space> From<Address<'space>> for Block {
    fn from(t: Address<'space>) -> Block {
        Self(t.offset() / BLOCK_SIZE)
    }
}

impl From<u64> for Block {
    fn from(t: u64) -> Block {
        Self(t)
    }
}

impl Block {
    #[inline]
    fn bit(&self) -> usize {
        self.0 as usize % size_of::<Self>()
    }

    #[inline]
    fn index(&self) -> usize {
        self.0 as usize / size_of::<Self>()
    }

    #[inline]
    fn start_address<'space>(&self, space: &'space AddressSpace) -> Address<'space> {
        Address::new(space, self.0 * BLOCK_SIZE)
    }

    #[inline]
    fn end_address<'space>(&self, space: &'space AddressSpace) -> Address<'space> {
        Address::new(space, (self.0 + 1) * BLOCK_SIZE)
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
struct DirtyBacking {
    indices: Vec<Block>,
    bitsmap: Vec<u64>,
}

impl DirtyBacking {
    fn new(size: usize) -> Self {
        let backing_size = 1 + (size as u64 / BLOCK_SIZE) as usize;
        Self {
            indices: Vec::with_capacity(backing_size),
            bitsmap: vec![0 as u64; 1 + backing_size / size_of::<u64>()],
        }
    }

    #[inline]
    fn fork(&self) -> Self {
        Self {
            indices: Vec::with_capacity(self.indices.capacity()),
            bitsmap: vec![0 as u64; self.bitsmap.len()],
        }
    }

    #[inline]
    fn dirty<B: Into<Block>>(&mut self, block: B) {
        let block = block.into();
        let index = block.index();
        let check = 1 << block.bit();

        if self.bitsmap[index] & check == 0 {
            self.bitsmap[index] |= check;
            self.indices.push(block);
        }
    }

    #[inline]
    fn dirty_region<'space>(&mut self, start: &Address<'space>, size: usize) {
        let sblock = Block::from(start).0;
        let eblock = Block::from(start.offset() + size as u64).0;

        for block in sblock..=eblock {
            self.dirty(block);
        }
    }
}
