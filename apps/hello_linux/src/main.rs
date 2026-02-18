#![no_std]
#![no_main]

use ftl::application::Event;
use ftl::application::EventLoop;
use ftl::error::ErrorCode;
use ftl::handle::HandleId;
use ftl::handle::Handleable;
use ftl::prelude::*;
use ftl::rc::Rc;
use ftl::syscall::sys_console_write;
use ftl::thread::Thread;
use ftl::thread::thread_resume_with;
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
        let mut pos = 0;
        while pos < buf.len() {
            let cur = addr + pos;
            let vma = self
                .vmas
                .iter()
                .find(|vma| vma.base <= cur && cur < vma.end)
                .ok_or(Error::NotMappedAddr)?;
            let span = core::cmp::min(buf.len() - pos, vma.end - cur);
            vma.vmarea
                .read(cur - vma.base, &mut buf[pos..pos + span])
                .map_err(Error::ReadVmArea)?;
            pos += span;
        }

        Ok(())
    }

    pub fn return_from_syscall(&self, thread_id: HandleId, retval: usize) {
        thread_resume_with(thread_id, retval).unwrap();
    }
}

#[ftl::main]
fn main() {
    info!("starting hello_linux");

    let mut eventloop = EventLoop::new().unwrap();
    let proc = LxProcess::create(&mut eventloop).unwrap();
    info!("thread started");

    const SYS_WRITE: u64 = 1;
    const SYS_EXIT: u64 = 60;

    loop {
        match eventloop.wait() {
            Event::Syscall { thread, regs } => {
                let retval = match regs.rax {
                    SYS_WRITE => {
                        let fd = regs.rdi as usize;
                        let addr = regs.rsi as usize;
                        let len = regs.rdx as usize;
                        let mut buf = vec![0; 4096];
                        let addr_aligned = (addr & !0xfff);
                        trace!(
                            "reading from user: addr={:x}, addr_aligned={:x}",
                            addr, addr_aligned
                        );
                        proc.read_from_user(addr_aligned, &mut buf).unwrap();

                        let offset = addr - addr_aligned;
                        let buf = &buf[offset..offset + len];

                        if fd == 1 || fd == 2 {
                            info!(
                                "[{}] {}",
                                if fd == 1 { "stdout" } else { "stderr" },
                                core::str::from_utf8(buf).unwrap()
                            );
                        }
                        len
                    }
                    SYS_EXIT => {
                        info!("linux process exited");
                        0
                    }
                    _ => {
                        warn!("unsupported linux syscall: {}", regs.rax);
                        0
                    }
                };

                proc.return_from_syscall(thread.handle().id(), retval);
            }
            _ => {}
        }
    }
}
