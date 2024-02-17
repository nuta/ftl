#![no_std]

use ftl_api::channel::Channel;
use ftl_api::device::mmio::ReadWrite;
use ftl_api::environ::Environ;
use ftl_api::folio::Folio;
use ftl_api::println;
use ftl_api::sync::Arc;
use ftl_api::sync::SpinLock;
use ftl_api::types::address::PAddr;
use ftl_api::types::signal::Signal;
use hashbrown::HashMap;

// TODO: Register definitions are incomplete. We need to save the memory fooprint
//       we should instantiate the registers dynamically.

static PRIORITY_REGS: &[ReadWrite<u32>] = &[
    ReadWrite::<u32>::new(0x0000_0004 + 4 * 0),
    ReadWrite::<u32>::new(0x0000_0004 + 4 * 1),
    ReadWrite::<u32>::new(0x0000_0004 + 4 * 2),
    ReadWrite::<u32>::new(0x0000_0004 + 4 * 3),
];

static ENABLE_REGS: &[ReadWrite<u32>] = &[
    ReadWrite::<u32>::new(0x0000_2080 + 4 * 0),
    ReadWrite::<u32>::new(0x0000_2080 + 4 * 1),
    ReadWrite::<u32>::new(0x0000_2080 + 4 * 2),
    ReadWrite::<u32>::new(0x0000_2080 + 4 * 3),
    ReadWrite::<u32>::new(0x0000_2080 + 4 * 4),
    ReadWrite::<u32>::new(0x0000_2080 + 4 * 5),
    ReadWrite::<u32>::new(0x0000_2080 + 4 * 6),
    ReadWrite::<u32>::new(0x0000_2080 + 4 * 7),
];

static THRESHOLD_REGS: &[ReadWrite<u32>] = &[
    ReadWrite::<u32>::new(0x201000 + 0x2000 * 0),
    ReadWrite::<u32>::new(0x201000 + 0x2000 * 1),
    ReadWrite::<u32>::new(0x201000 + 0x2000 * 2),
    ReadWrite::<u32>::new(0x201000 + 0x2000 * 3),
];

static CLAIM_REGS: &[ReadWrite<u32>] = &[
    ReadWrite::<u32>::new(0x201004 + 0x2000 * 0),
    ReadWrite::<u32>::new(0x201004 + 0x2000 * 1),
    ReadWrite::<u32>::new(0x201004 + 0x2000 * 2),
    ReadWrite::<u32>::new(0x201004 + 0x2000 * 3),
];

#[derive(Debug)]
enum PlicError {
    IrqOutOfRange,
}

struct Plic {
    mmio: Folio,
}

impl Plic {
    pub fn new(mmio: Folio) -> Plic {
        Plic { mmio }
    }

    pub fn enable_irq(&mut self, irq: usize) -> Result<(), PlicError> {
        println!("plic: enabling irq {}", irq);

        let priority_reg = PRIORITY_REGS.get(irq).ok_or(PlicError::IrqOutOfRange)?;
        priority_reg.write(&mut self.mmio, 1);

        // Enable IRQ.
        let enable_reg = ENABLE_REGS.get(irq / 32).ok_or(PlicError::IrqOutOfRange)?;
        let value = enable_reg.read(&mut self.mmio);
        enable_reg.write(&mut self.mmio, value | (1 << (irq % 32)));

        Ok(())
    }

    pub fn read_pending_irq(&mut self, hart: usize) -> Result<Option<u32>, PlicError> {
        debug_assert!(hart < 64);

        let claim_reg = CLAIM_REGS.get(hart).ok_or(PlicError::IrqOutOfRange)?;
        let irq = claim_reg.read(&mut self.mmio);
        if irq == 0 {
            Ok(None)
        } else {
            Ok(Some(irq))
        }
    }

    pub fn ack_irq(&mut self, irq: u32) -> Result<(), PlicError> {
        let claim_reg = CLAIM_REGS.get(0).ok_or(PlicError::IrqOutOfRange)?;
        claim_reg.write(&mut self.mmio, irq);
        Ok(())
    }

    pub fn init_hart(&mut self, hart: usize) -> Result<(), PlicError> {
        // Set priority threshold to 0 to accept all interrupts.
        let threshold_reg = THRESHOLD_REGS.get(hart).ok_or(PlicError::IrqOutOfRange)?;
        threshold_reg.write(&mut self.mmio, 0);
        Ok(())
    }
}

struct Listeners {
    irqs: HashMap<usize, Option<Channel>>,
}

impl Listeners {
    pub fn new() -> Listeners {
        Listeners {
            irqs: HashMap::new(),
        }
    }

    pub fn notify_irq(&mut self, irq: usize) {
        if let Some(channel) = self.irqs.get_mut(&irq) {
            if let Some(channel) = channel {
                channel.notify(Signal::Interrupt).unwrap();
            }
        }
    }

    pub fn add_listener(&mut self, irq: usize, channel: Channel) {
        debug_assert!(!self.irqs.contains_key(&irq)); // TODO:
        self.irqs.insert(irq, Some(channel));
    }
}

pub fn main(env: Environ) {
    println!("plic: starting: {:?}", env.device());
    let base_paddr = PAddr::new(env.device().reg as usize).unwrap();
    let folio = Folio::map_paddr(base_paddr, 0x4000000).unwrap();
    let plic = Arc::new(SpinLock::new(Plic::new(folio)));
    let listeners = Arc::new(SpinLock::new(Listeners::new()));

    // Interrupt handler.
    {
        let plic = plic.clone();
        let listeners = listeners.clone();
        ftl_kernel_api::listen_for_hardware_interrupts(move || {
            let hart = ftl_kernel_api::get_cpu_id();
            let mut plic = plic.lock();
            let mut listeners = listeners.lock();
            loop {
                let irq = match plic.read_pending_irq(hart) {
                    Ok(Some(irq)) => irq,
                    Ok(None) => break,
                    Err(e) => {
                        println!("plic: error reading pending irq: {:?}", e);
                        break;
                    }
                };

                listeners.notify_irq(irq as usize);
                plic.ack_irq(irq).unwrap();
            }
        });
    }

    // Per-hart initialization.
    {
        let plic = plic.clone();
        ftl_kernel_api::init_per_cpu(move |id| {
            plic.lock().init_hart(id).unwrap();
        });
    }

    // TODO: EventPoll to handle enable_irq requests
}
