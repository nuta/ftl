#![no_std]

use ftl_api::{environ::Environ, folio::Folio, println};

struct ReadWrite<T> {
    offset: usize,
    _pd: core::marker::PhantomData<T>,
}

impl ReadWrite<u32> {
    pub const fn new(offset: usize) -> ReadWrite<u32> {
        ReadWrite {
            offset,
            _pd: core::marker::PhantomData,
        }
    }

    pub fn read(&self) -> u32 {
        todo!()
    }

    pub fn write(&self, value: u32) {
        todo!()
    }
}

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

    pub fn enable_irq(&self, irq: usize) -> Result<(), PlicError> {
        println!("plic: enabling irq {}", irq);

        let priority_reg = PRIORITY_REGS.get(irq).ok_or(PlicError::IrqOutOfRange)?;
        priority_reg.write(1);

        // Enable IRQ.
        let enable_reg = ENABLE_REGS.get(irq / 32).ok_or(PlicError::IrqOutOfRange)?;
        let value = enable_reg.read();
        enable_reg.write(value | (1 << (irq % 32)));

        Ok(())
    }

    pub fn read_pending_irq(&self, hart: usize) -> Result<Option<u32>, PlicError> {
        debug_assert!(hart < 64);

        let claim_reg = CLAIM_REGS.get(hart).ok_or(PlicError::IrqOutOfRange)?;
        let irq = claim_reg.read();
        if irq == 0 {
            Ok(None)
        } else {
            Ok(Some(irq))
        }
    }

    pub fn ack_irq(&self, irq: u32) -> Result<(), PlicError> {
        let claim_reg = CLAIM_REGS.get(0).ok_or(PlicError::IrqOutOfRange)?;
        claim_reg.write(irq);
        Ok(())
    }

    pub fn init_hart(&self, hart: usize) -> Result<(), PlicError> {
        // Set priority threshold to 0 to accept all interrupts.
        let threshold_reg = THRESHOLD_REGS.get(hart).ok_or(PlicError::IrqOutOfRange)?;
        threshold_reg.write(0);
        Ok(())
    }
}

pub fn main(env: Environ) {
    println!("plic: starting: {:?}", env.device());
}
