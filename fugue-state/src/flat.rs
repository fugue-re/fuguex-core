use std::fmt;
use std::mem::size_of;

use fugue::ir::IntoAddress;
use fugue::ir::{Address, AddressSpace};

use crate::traits::State;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error<'space> {
    #[error("{access} access violation at {address} of {size} bytes in `{}` space", address.space().name())]
    AccessViolation { address: Address<'space>, size: usize, access: Access },
    #[error("out-of-bounds read of `{size}` bytes at {address}")]
    OOBRead { address: Address<'space>, size: usize },
    #[error("out-of-bounds write of `{size}` bytes at {address}")]
    OOBWrite { address: Address<'space>, size: usize },
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct FlatState<'space> {
    backing: Vec<u8>,
    dirty: DirtyBacking,
    permissions: Permissions,
    space: &'space AddressSpace,
}

impl<'space> AsRef<Self> for FlatState<'space> {
    #[inline(always)]
    fn as_ref(&self) -> &Self {
        self
    }
}

impl<'space> AsMut<Self> for FlatState<'space> {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut Self {
        self
    }
}

impl<'space> FlatState<'space> {
    pub fn new(space: &'space AddressSpace, size: usize) -> Self {
        Self {
            backing: vec![0u8; size],
            dirty: DirtyBacking::new(size),
            permissions: Permissions::new(size),
            space,
        }
    }

    pub fn read_only(space: &'space AddressSpace, size: usize) -> Self {
        Self {
            backing: vec![0u8; size],
            dirty: DirtyBacking::new(size),
            permissions: Permissions::new_with(size, PERM_READ_MASK),
            space,
        }
    }

    pub fn from_vec(space: &'space AddressSpace, bytes: Vec<u8>) -> Self {
        let size = bytes.len();
        Self {
            backing: bytes,
            dirty: DirtyBacking::new(size),
            permissions: Permissions::new(size),
            space,
        }
    }

    pub fn permissions(&self) -> &Permissions {
        &self.permissions
    }

    pub fn permissions_mut(&mut self) -> &mut Permissions {
        &mut self.permissions
    }
}

impl<'space> State for FlatState<'space> {
    type Error = Error<'space>;

    fn fork(&self) -> Self {
        Self {
            backing: self.backing.clone(),
            dirty: self.dirty.fork(),
            permissions: self.permissions.clone(),
            space: self.space,
        }
    }

    fn restore(&mut self, other: &Self) {
        for block in &self.dirty.indices {
            let start = block.start_address(self.space).offset() as usize;
            let end = block.end_address(self.space).offset() as usize;

            let real_end = self.backing.len().min(end);

            self.dirty.bitsmap[block.index()] = 0;
            self.backing[start..real_end].copy_from_slice(&other.backing[start..real_end]);
        }
        self.permissions = other.permissions.clone();
    }

    fn copy_bytes<F, T>(&mut self, from: F, to: T, size: usize) -> Result<(), Error<'space>>
    where F: IntoAddress,
          T: IntoAddress {
        let from = from.into_address(self.space);
        let to = to.into_address(self.space);

        let soff = from.offset() as usize;
        let doff = to.offset() as usize;

        if soff > self.len() || soff.checked_add(size).is_none() || soff + size > self.len() {
            return Err(Error::OOBRead {
                address: from.clone(),
                size, //(soff + size) - self.len(),
            });
        }

        if !self.permissions.all_readable(&from, size) {
            return Err(Error::AccessViolation {
                address: from.clone(),
                size,
                access: Access::Read,
            })
        }

        if doff > self.len() || doff.checked_add(size).is_none() || doff + size > self.len() {
            return Err(Error::OOBWrite {
                address: to.clone(),
                size, // (doff + size) - self.len(),
            });
        }

        if !self.permissions.all_writable(&to, size) {
            return Err(Error::AccessViolation {
                address: to.clone(),
                size,
                access: Access::Write,
            })
        }

        self.backing.copy_within(soff..(soff + size), doff);
        self.dirty.dirty_region(&to, size);

        Ok(())
    }

    fn get_bytes<A>(&self, address: A, bytes: &mut [u8]) -> Result<(), Error<'space>>
    where A: IntoAddress {
        let address = address.into_address(self.space);
        let size = bytes.len();
        let start = address.offset() as usize;
        let end = start.checked_add(size);

        if start > self.len() || end.is_none() || end.unwrap() > self.len() {
            return Err(Error::OOBRead {
                address: address.clone(),
                size: bytes.len(),
            });
        }

        if !self.permissions.all_readable(&address, size) {
            return Err(Error::AccessViolation {
                address: address.clone(),
                size,
                access: Access::Read,
            })
        }

        let end = end.unwrap();

        bytes[..].copy_from_slice(&self.backing[start..end]);

        Ok(())
    }

    fn view_bytes<A>(&self, address: A, size: usize) -> Result<&[u8], Error<'space>>
    where A: IntoAddress {
        let address = address.into_address(self.space);
        let start = address.offset() as usize;
        let end = start.checked_add(size);

        if start > self.len() || end.is_none() || end.unwrap() > self.len() {
            return Err(Error::OOBRead {
                address: address.clone(),
                size,
            });
        }

        if !self.permissions.all_readable(&address, size) {
            return Err(Error::AccessViolation {
                address: address.clone(),
                size,
                access: Access::Read,
            })
        }

        let end = end.unwrap();

        Ok(&self.backing[start..end])
    }

    fn view_bytes_mut<A>(&mut self, address: A, size: usize) -> Result<&mut [u8], Error<'space>>
    where A: IntoAddress {
        let address = address.into_address(self.space);
        let start = address.offset() as usize;
        let end = start.checked_add(size);

        if start > self.len() || end.is_none() || end.unwrap() > self.len() {
            return Err(Error::OOBRead {
                address: address.clone(),
                size,
            });
        }

        if !self.permissions.all_readable_and_writable(&address, size) {
            return Err(Error::AccessViolation {
                address: address.clone(),
                size,
                access: Access::ReadWrite,
            })
        }

        let end = end.unwrap();

        self.dirty.dirty_region(&address, size);

        Ok(&mut self.backing[start..end])
    }

    fn set_bytes<A>(&mut self, address: A, bytes: &[u8]) -> Result<(), Error<'space>>
    where A: IntoAddress {
        let address = address.into_address(self.space);
        let size = bytes.len();
        let start = address.offset() as usize;
        let end = start.checked_add(size);

        if start > self.len() || end.is_none() || end.unwrap() > self.len() {
            return Err(Error::OOBWrite {
                address: address.clone(),
                size,
            });
        }

        if !self.permissions.all_writable(&address, size) {
            return Err(Error::AccessViolation {
                address: address.clone(),
                size,
                access: Access::Write,
            })
        }

        let end = end.unwrap();

        self.backing[start..end].copy_from_slice(bytes);
        self.dirty.dirty_region(&address, size);

        Ok(())
    }

    fn len(&self) -> usize {
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

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum Access {
    Read,
    Write,
    ReadWrite,
}

impl fmt::Display for Access {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Access::Read => write!(f, "read"),
            Access::Write => write!(f, "write"),
            Access::ReadWrite => write!(f, "read/write")
        }
    }
}

impl Access {
    #[inline]
    pub fn is_read(&self) -> bool {
        matches!(self, Access::Read)
    }

    #[inline]
    pub fn is_write(&self) -> bool {
        matches!(self, Access::Write)
    }

    #[inline]
    pub fn is_read_write(&self) -> bool {
        matches!(self, Access::ReadWrite)
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Permissions {
    bitsmap: Vec<u64>,
}

const PERM_READ_OFF: usize = 1;
const PERM_WRITE_OFF: usize = 0;
const PERM_READ_WRITE_OFF: usize = 0;

const PERM_READ_MASK: u64 = 0xAAAAAAAAAAAAAAAA;
const PERM_WRITE_MASK: u64 = 0x5555555555555555;

const PERM_SCALE: usize = (size_of::<u64>() << 3) >> 1;
const PERM_SELECT: usize = 1;

impl Permissions {
    pub fn new(size: usize) -> Self {
        Self::new_with(size, PERM_READ_MASK | PERM_WRITE_MASK)
    }

    #[inline]
    pub fn new_with(size: usize, mask: u64) -> Self {
        Self {
            // NOTE: we represent each permission by two bits and set
            // each byte to readable by default
            bitsmap: vec![mask; 1 + size / PERM_SCALE],
        }
    }

    #[inline]
    pub fn is_marked<'space>(&self, address: &Address<'space>, access: Access) -> bool {
        let address = address.offset();
        let index = (address / PERM_SCALE as u64) as usize;
        let bit = ((address % PERM_SCALE as u64) as usize) << PERM_SELECT;
        let check = if access.is_read_write() {
            0b11 << (bit + PERM_READ_WRITE_OFF)
        } else {
            1 << if access.is_read() {
                bit + PERM_READ_OFF
            } else {
                bit + PERM_WRITE_OFF
            }
        };

        self.bitsmap[index] & check == check
    }

    #[inline]
    pub fn is_readable<'space>(&self, address: &Address<'space>) -> bool {
        self.is_marked(address, Access::Read)
    }

    #[inline]
    pub fn is_writable<'space>(&self, address: &Address<'space>) -> bool {
        self.is_marked(address, Access::Write)
    }

    #[inline]
    pub fn is_readable_and_writable<'space>(&self, address: &Address<'space>) -> bool {
        self.is_marked(address, Access::ReadWrite)
    }

    #[inline]
    pub fn all_marked<'space>(&self, address: &Address<'space>, size: usize, access: Access) -> bool {
        let start = address.offset();
        for addr in start..(start + size as u64) {
            if !self.is_marked(&Address::new(address.space(), addr), access) {
                return false
            }
        }
        true
    }

    #[inline]
    pub fn all_readable<'space>(&self, address: &Address<'space>, size: usize) -> bool {
        self.all_marked(address, size, Access::Read)
    }

    #[inline]
    pub fn all_writable<'space>(&self, address: &Address<'space>, size: usize) -> bool {
        self.all_marked(address, size, Access::Write)
    }

    #[inline]
    pub fn all_readable_and_writable<'space>(&self, address: &Address<'space>, size: usize) -> bool {
        self.all_marked(address, size, Access::ReadWrite)
    }

    #[inline]
    pub fn clear_byte<'space>(&mut self, address: &Address<'space>, access: Access) {
        let address = address.offset();
        let index = (address / PERM_SCALE as u64) as usize;
        let bit = ((address % PERM_SCALE as u64) as usize) << PERM_SELECT;
        let check = if access.is_read_write() {
            0b11 << (bit + PERM_READ_WRITE_OFF)
        } else {
            1 << if access.is_read() {
                bit + PERM_READ_OFF
            } else {
                bit + PERM_WRITE_OFF
            }
        };
        self.bitsmap[index] &= !check;
    }

    #[inline]
    pub fn set_byte<'space>(&mut self, address: &Address<'space>, access: Access) {
        let address = address.offset();
        let index = (address / PERM_SCALE as u64) as usize;
        let bit = ((address % PERM_SCALE as u64) as usize) << PERM_SELECT;
        let check = if access.is_read_write() {
            0b11 << (bit + PERM_READ_WRITE_OFF)
        } else {
            1 << if access.is_read() {
                bit + PERM_READ_OFF
            } else {
                bit + PERM_WRITE_OFF
            }
        };

        self.bitsmap[index] |= check;
    }

    #[inline]
    pub fn clear_region<'space>(&mut self, address: &Address<'space>, size: usize, access: Access) {
        let start = address.offset();
        for addr in start..(start + size as u64) {
            self.clear_byte(&Address::new(address.space(), addr), access);
        }
    }

    #[inline]
    pub fn set_region<'space>(&mut self, address: &Address<'space>, size: usize, access: Access) {
        let start = address.offset();
        for addr in start..(start + size as u64) {
            self.set_byte(&Address::new(address.space(), addr), access);
        }
    }
}
