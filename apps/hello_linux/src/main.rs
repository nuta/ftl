#![no_std]
#![no_main]

use ftl::application::Event;
use ftl::application::EventLoop;
use ftl::error::ErrorCode;
use ftl::prelude::*;
use ftl::rc::Rc;
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
    AddThreadToEventLoop(ErrorCode),
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
    threads: Vec<Rc<Thread>>,
    vmspace: VmSpace,
    vmas: Vec<Vma>,
}
impl LxProcess {
    pub fn create(eventloop: &mut EventLoop) -> Result<Self, Error> {
        let vmspace: VmSpace = VmSpace::new().map_err(Error::CreateVmSpace)?;
        let process = ftl::process::Process::create_sandboxed(&vmspace, "hello_linux")
            .map_err(Error::CreateProcess)?;

        const SYSCALL_BIN: &[u8] = include_bytes!("../syscall.bin");
        trace!("syscall.bin size: {}", SYSCALL_BIN.len());
        let base = 0x1000000;
        let entry = 0x1000000;
        let sp = 0xdeadbeef;

        let thread = Thread::create(&process, entry, sp, 0).map_err(Error::CreateThread)?;

        let vmarea = VmArea::new(4096).map_err(Error::CreateVmArea)?;
        vmarea.write(0, SYSCALL_BIN).map_err(Error::WriteVmArea)?;

        vmspace
            .map(&vmarea, base, PageAttrs::WRITABLE)
            .map_err(Error::MapVmArea)?;

        thread.start().map_err(Error::StartThread)?;

        let thread = Rc::new(thread);
        eventloop
            .add_thread(thread.clone())
            .map_err(Error::AddThreadToEventLoop)?;

        Ok(Self {
            ftl_process: process,
            threads: vec![thread],
            vmspace,
            vmas: vec![Vma::new(vmarea, base, base + SYSCALL_BIN.len())],
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
    let _proc = LxProcess::create(&mut eventloop).unwrap();
    // eventloop.add_thread(&proc.threads[0]).unwrap();
    info!("thread started");

    loop {
        match eventloop.wait() {
            Event::Syscall { thread, regs } => {
                info!(
                    "syscall event: rax={:x}, rdi={:x}, rsi={:x}, rdx={:x}, r10={:x}, r8={:x}, r9={:x}",
                    regs.rax, regs.rdi, regs.rsi, regs.rdx, regs.r10, regs.r8, regs.r9
                );
            }
            _ => {}
        }
    }
}
