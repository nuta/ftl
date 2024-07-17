#![no_std]
#![no_main]

use ftl_api::channel::Channel;
use ftl_api::driver::interrupt_controller::set_interrupt_handler;
use ftl_api::driver::mmio::LittleEndian;
use ftl_api::driver::mmio::MmioReg;
use ftl_api::driver::mmio::ReadOnly;
use ftl_api::driver::mmio::ReadWrite;
use ftl_api::folio::MmioFolio;
use ftl_api::handle::OwnedHandle;
use ftl_api::mainloop::Event;
use ftl_api::mainloop::Mainloop;
use ftl_api::prelude::*;
use ftl_api::signal::Signal;
use ftl_api::types::address::PAddr;
use ftl_api::types::address::VAddr;
use ftl_api::types::environ::Device;
use ftl_api::types::message::MessageBuffer;
use ftl_api_autogen::apps::arm_gic::Environ;
use ftl_api_autogen::apps::arm_gic::Message;
use ftl_api_autogen::protocols::intc::ListenReply;
use spin::mutex::SpinMutex;
use spin::Mutex;

// > In the GIC architecture, all registers that are halfword-accessible or
// > byte-accessible use a little endian memory order model.
// >
// > 4.1.4 GIC register access

/// Distributor Control Register.
const GICD_CTLR: MmioReg<LittleEndian, ReadWrite, u32> = MmioReg::new(0x000);
/// Interrupt Controller Type Register.
const GICD_TYPER: MmioReg<LittleEndian, ReadOnly, u32> = MmioReg::new(0x004);
/// Interrupt Set-Enable Registers.
#[allow(non_upper_case_globals)]
const GICD_IENABLERn: MmioReg<LittleEndian, ReadWrite, u32> = MmioReg::new(0x100);
/// Interrupt Priority Registers.
#[allow(non_upper_case_globals)]
const GICD_IPRIORITYRn: MmioReg<LittleEndian, ReadWrite, u32> = MmioReg::new(0x400);
/// Interrupt Processor Targets Registers.
#[allow(non_upper_case_globals)]
const GICD_ITARGETSRn: MmioReg<LittleEndian, ReadWrite, u32> = MmioReg::new(0x800);
/// CPU Interface Control Register,
const GICC_CTLR: MmioReg<LittleEndian, ReadWrite, u32> = MmioReg::new(0x000);
/// Interrupt Priority Mask Register.
const GICC_PMR: MmioReg<LittleEndian, ReadWrite, u32> = MmioReg::new(0x004);

struct Gic {
    gicd_folio: MmioFolio,
    gicc_folio: MmioFolio,
}

impl Gic {
    pub fn init_device(mut dist_folio: MmioFolio, mut cpu_folio: MmioFolio) -> Self {
        // Reset the device.
        GICD_CTLR.write(&mut dist_folio, 0);
        // Determine the maximum number of interrupts (ITLinesNumber field).
        trace!("reading GICD_TYPER");
        let it_lines_number = GICD_TYPER.read(&mut dist_folio) & 0b1111;
        trace!("read GICD_TYPER");
        let num_max_intrs = (it_lines_number + 1) * 32;

        trace!("GIC: max # of IRQs = {}", num_max_intrs);
        GICC_PMR.write(&mut cpu_folio, 255);

        GICC_CTLR.write(&mut cpu_folio, 1);
        GICD_CTLR.write(&mut dist_folio, 1);

        Self {
            gicd_folio: dist_folio,
            gicc_folio: cpu_folio,
        }
    }

    pub fn enable_irq(&mut self, irq: usize) {
        let irq_shift = (irq % 4) * 8;

        // Enable the interrupt.
        {
            let offset = irq / 32;
            let mut value = GICD_IENABLERn.read_with_offset(&mut self.gicd_folio, offset);
            value |= 1 << (irq % 32);
            GICD_IENABLERn.write_with_offset(&mut self.gicd_folio, offset, value);
        }

        // Set the priority of the interrupt to the highest.
        {
            let offset = irq / 4;
            let mut value = GICD_IPRIORITYRn.read_with_offset(&mut self.gicd_folio, offset);
            value &= !(0xff << irq_shift);
            GICD_IPRIORITYRn.write_with_offset(&mut self.gicd_folio, offset, value);
        }

        // Set the target processor to the first processor.
        // TODO: Multi-processor support.
        {
            let target = 0; /* CPU interface 0 */
            let offset = irq / 4;
            let mut value = GICD_ITARGETSRn.read_with_offset(&mut self.gicd_folio, offset);
            value &= !(0xff << irq_shift);
            value |= (1 << target) << irq_shift;
            GICD_ITARGETSRn.write_with_offset(&mut self.gicd_folio, offset, value);
        }
    }
}

enum Context {
    Autopilot,
    Client,
}

#[ftl_api::main]
pub fn main(mut env: Environ) {
    let mut mainloop = Mainloop::<Context, Message>::new().unwrap();
    mainloop
        .add_channel(env.autopilot_ch.take().unwrap(), Context::Autopilot)
        .unwrap();

    let gic = env.depends.gic.get_mut(0).take().unwrap();
    info!("starting arm_gic: {:?}", gic);

    let gicd_paddr: usize = gic.reg.try_into().unwrap();
    let gicd_folio = MmioFolio::create_pinned(PAddr::new(gicd_paddr).unwrap(), 0x1000).unwrap();
    let gicc_folio = MmioFolio::create_pinned(
        PAddr::new(gicd_paddr + 0x10000 /* FIXME: */).unwrap(),
        0x1000,
    )
    .unwrap();
    let mut gic = Gic::init_device(gicd_folio, gicc_folio);
    let listeners = Arc::new(Mutex::new(HashMap::new()));


    info!("--------------------------------------------");
    gic.enable_irq(0);
    gic.enable_irq(1);
    gic.enable_irq(27);
    gic.enable_irq(283);
    unsafe {
        core::arch::asm!("msr cntv_ctl_el0, {}", in(reg) (0));
        core::arch::asm!("msr cntv_cval_el0, {}", in(reg) (1000000));
        core::arch::asm!("msr cntv_ctl_el0, {}", in(reg) (1));
    }

    set_interrupt_handler(|| {
        info!("caught interrupt!");
    }).unwrap();


    let mut buffer = MessageBuffer::new();
    loop {
        match mainloop.next(&mut buffer) {
            Event::Message { ctx, ch, m } => {
                match m {
                    Message::NewclientRequest(m) => {
                        info!("got new client: {:?}", m.handle());
                        let new_ch = Channel::from_handle(OwnedHandle::from_raw(m.handle()));
                        mainloop
                            .add_channel(new_ch, Context::Client)
                            .unwrap();
                    }
                    Message::ListenRequest(m) => {
                        let irq = m.irq();
                        let signal = Signal::from_handle(OwnedHandle::from_raw(m.signal()));
                        info!("listen request: {:?}", irq);
                        gic.enable_irq(irq as usize);
                        listeners.lock().insert(irq, signal);

                        let _ = ch.send_with_buffer(&mut buffer, ListenReply {});
                    }
                    _ => {
                        warn!("unhandled message, {:?}", m);
                    }
                }
            }
            _ => {
                warn!("unhandled event");
            }
        }
    }
}
