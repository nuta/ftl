#![no_std]
#![no_main]

use ftl::application::Event;
use ftl::application::EventLoop;
use ftl::error::ErrorCode;
use ftl::prelude::*;
use ftl::thread::Thread;
use ftl::vmarea::VmArea;
use ftl::vmspace::PageAttrs;
use ftl::vmspace::VmSpace;

#[derive(Debug)]
enum Error {
    CreateProcess(ErrorCode),
    CreateThread(ErrorCode),
    CreateVmSpace(ErrorCode),
    CreateVmArea(ErrorCode),
    MapVmArea(ErrorCode),
    WriteVmArea(ErrorCode),
    ReadVmArea(ErrorCode),
    NotMappedAddr,
    StartThread(ErrorCode),
}

struct Vma {
    vmarea: VmArea,
    base: usize,
    end: usize,
}

impl Vma {
    pub fn new(vmarea: VmArea, base: usize, end: usize) -> Self {
        Self { vmarea, base, end }
    }
}

struct LxProcess {
    ftl_process: ftl::process::Process,
    threads: Vec<Thread>,
    vmspace: VmSpace,
    vmas: Vec<Vma>,
}
impl LxProcess {
    pub fn create() -> Result<Self, Error> {
        let vmspace = VmSpace::new().map_err(Error::CreateVmSpace)?;
        let process = ftl::process::Process::create_sandboxed(&vmspace, "hello_linux")
            .map_err(Error::CreateProcess)?;

        const HELLO_WORLD_BIN: &[u8] = include_bytes!("../../../hello_world.bin");
        trace!("hello_world.bin size: {}", HELLO_WORLD_BIN.len());
        let base = 0x1000000;
        let entry = 0x1001260;

        let thread = Thread::create(&process, entry, sp, 0).map_err(Error::CreateThread)?;

        let vmarea = VmArea::new(4096).map_err(Error::CreateVmArea)?;
        vmarea
            .write(0, HELLO_WORLD_BIN)
            .map_err(Error::WriteVmArea)?;

        vmspace
            .map(&vmarea, base, PageAttrs::WRITABLE)
            .map_err(Error::MapVmArea)?;

        thread.start().map_err(Error::StartThread)?;

        Ok(Self {
            ftl_process: process,
            threads: vec![thread],
            vmspace,
            vmas: vec![Vma::new(vmarea, base, base + HELLO_WORLD_BIN.len())],
        })
    }

    pub fn read_from_user(&self, addr: usize, buf: &mut [u8]) -> Result<(), Error> {
        let vma = self
            .vmas
            .iter()
            .find(|vma| vma.base <= addr && addr < vma.end)
            .ok_or(Error::NotMappedAddr)?;

        vma.vmarea
            .read(addr - vma.base, buf)
            .map_err(Error::ReadVmArea)
    }

    pub fn return_from_syscall(&self, retval: usize) {
        todo!()
    }
}

#[ftl::main]
fn main() {
    info!("starting hello_linux");

    let mut eventloop = EventLoop::new().unwrap();
    let proc = LxProcess::create().unwrap();
    // eventloop.add_thread(&proc.thread()).unwrap();
    info!("thread started");

    loop {
        match eventloop.wait() {
            Event::Syscall { regs } => {
                const SYS_WRITE: u64 = 1;
                match regs.rax {
                    SYS_WRITE => {
                        let fd = regs.rdi as usize;
                        let buf = regs.rsi as usize;
                        let len = regs.rdx as usize;
                        info!("write: fd={}, buf={:x}, len={}", fd, buf, len);

                        let mut tmp = [0; 32];
                        proc.read_from_user(buf, &mut tmp).unwrap();
                        info!("write: buf={:?}", core::str::from_utf8(&tmp));

                        proc.return_from_syscall(len);
                    }
                    _ => {
                        info!(
                            "syscall: rax={:x}, rdi={:x}, rsi={:x}, rdx={:x}, r10={:x}, r8={:x}, r9={:x}",
                            regs.rax, regs.rdi, regs.rsi, regs.rdx, regs.r10, regs.r8, regs.r9
                        );
                    }
                }
            }
            _ => {}
        }
    }
}
