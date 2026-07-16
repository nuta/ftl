use alloc::vec::Vec;
use core::mem::size_of;

use ftl_api::error::ErrorCode;
use ftl_api::vmarea::VmArea;
use ftl_elf::Elf;
use ftl_utils::alignment::align_up;
use ftl_utils::alignment::is_aligned;

use crate::process::PAGE_SIZE;

pub const STACK_BASE: usize = 0x0000_7000_0000_0000;
pub const STACK_SIZE: usize = 64 * 1024;

const AT_NULL: usize = 0;
const AT_PHDR: usize = 3;
const AT_PHENT: usize = 4;
const AT_PHNUM: usize = 5;
const AT_PAGESZ: usize = 6;
const AT_ENTRY: usize = 9;
const AT_RANDOM: usize = 25;
const AT_EXECFN: usize = 31;

#[derive(Debug)]
pub enum Error {
    StackTooSmall,
    UnalignedWrite,
    NotNulTerminated,
    VmAreaWrite(ErrorCode),
}

struct StackWriter {
    buf: Vec<u8>,
    remaining: usize,
}

impl StackWriter {
    fn new(max_size: usize) -> Self {
        StackWriter {
            buf: Vec::new(),
            remaining: max_size,
        }
    }

    fn offset(&self) -> usize {
        self.buf.len()
    }

    fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), Error> {
        if self.remaining < bytes.len() {
            return Err(Error::StackTooSmall);
        }

        self.buf.extend_from_slice(bytes);
        self.remaining -= bytes.len();
        Ok(())
    }

    fn write_usize(&mut self, value: usize) -> Result<(), Error> {
        if !is_aligned(self.offset(), size_of::<usize>()) {
            return Err(Error::UnalignedWrite);
        }

        self.write_bytes(&value.to_ne_bytes())
    }

    fn align(&mut self, align: usize) -> Result<(), Error> {
        let padding_len = align_up(self.offset(), align) - self.offset();
        if self.remaining < padding_len {
            return Err(Error::StackTooSmall);
        }

        self.buf.resize(self.offset() + padding_len, 0);
        self.remaining -= padding_len;
        Ok(())
    }

    fn finish(self, vmarea: &VmArea) -> Result<(), Error> {
        vmarea
            .write(self.remaining, &self.buf)
            .map_err(Error::VmAreaWrite)
    }
}

struct Layout {
    sp: usize,
    argv_base: usize,
    env_base: usize,
    random_uaddr: usize,
}

fn estimate_stack_size(
    argv: &[&[u8]],
    env: &[&[u8]],
    random_len: usize,
    auxv_count: usize,
) -> Result<Layout, Error> {
    let mut offset = 0;
    offset += size_of::<usize>(); // argc
    offset += size_of::<usize>() * argv.len(); // argv[i]
    offset += size_of::<usize>(); // NULL
    offset += size_of::<usize>() * env.len(); // envp[i]
    offset += size_of::<usize>(); // NULL
    offset += size_of::<usize>() * auxv_count * 2; // auxv[i]

    let argv_offset = offset;
    for s in argv {
        if !s.ends_with(b"\0") {
            return Err(Error::NotNulTerminated);
        }
        offset += s.len();
    }

    let env_offset = offset;
    for s in env {
        if !s.ends_with(b"\0") {
            return Err(Error::NotNulTerminated);
        }
        offset += s.len();
    }

    let random_offset = offset;
    offset += random_len;

    let pad = align_up(offset, 16) - offset;
    let total = offset + pad;
    if total > STACK_SIZE {
        return Err(Error::StackTooSmall);
    }

    let sp = STACK_BASE + STACK_SIZE - total;
    Ok(Layout {
        sp,
        argv_base: sp + argv_offset,
        env_base: sp + env_offset,
        random_uaddr: sp + random_offset,
    })
}

pub fn build_initial_stack(
    stack: &VmArea,
    elf: &Elf,
    argv: &[&[u8]],
    env: &[&[u8]],
    phdr_uaddr: Option<u64>,
    random: &[u8; 16],
) -> Result<usize, Error> {
    // TODO: Every write does size checking. Can we optimize this?
    // TODO: auxvs depend on estimate_stack_size.
    const AUXV_COUNT: usize = 8;

    let layout = estimate_stack_size(argv, env, random.len(), AUXV_COUNT)?;
    let auxv = [
        (AT_RANDOM, layout.random_uaddr),
        (AT_EXECFN, layout.argv_base),
        (AT_PHDR, phdr_uaddr.unwrap_or(0) as usize),
        (AT_PHENT, elf.ehdr.e_phentsize as usize),
        (AT_PHNUM, elf.ehdr.e_phnum as usize),
        (AT_PAGESZ, PAGE_SIZE),
        (AT_ENTRY, elf.ehdr.e_entry as usize),
        (AT_NULL, 0),
    ];

    let mut w = StackWriter::new(STACK_SIZE);
    w.write_usize(argv.len())?; // argc

    let mut uaddr = layout.argv_base;
    for s in argv {
        w.write_usize(uaddr)?; // argv[i]
        uaddr += s.len();
    }
    w.write_usize(0)?; // NULL

    let mut uaddr = layout.env_base;
    for s in env {
        w.write_usize(uaddr)?; // envp[i]
        uaddr += s.len();
    }
    w.write_usize(0)?; // NULL

    // Auxiliary vectors.
    for &(tag, value) in &auxv {
        w.write_usize(tag)?;
        w.write_usize(value)?;
    }

    // Strings. argv/env are NULL-terminated (checked in estimate_stack_size).
    for s in argv {
        w.write_bytes(s)?;
    }
    for s in env {
        w.write_bytes(s)?;
    }
    w.write_bytes(random)?;

    // Align to 16 bytes, which is required by x86-64 ABI.
    w.align(16)?;

    w.finish(stack)?;
    Ok(layout.sp)
}
