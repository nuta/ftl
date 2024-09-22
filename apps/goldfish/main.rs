#![no_std]
#![no_main]

use ftl_api::types::address::PAddr;
use ftl_api::{environ::Environ, folio::MappedFolio};
use ftl_api::prelude::*;
use ftl_driver_utils::mmio::{LittleEndian, MmioReg, ReadOnly};

/// <https://github.com/qemu/qemu/blob/01dc65a3bc262ab1bec8fe89775e9bbfa627becb/hw/riscv/virt.c#L74>
const MMIO_BASE: PAddr =PAddr::new(0x101000);
const MMIO_SIZE: usize = 4096;
static TIME_LOW_REG: MmioReg<LittleEndian, ReadOnly, u32> = MmioReg::new(0x00);
static TIME_HIGH_REG: MmioReg<LittleEndian, ReadOnly, u32> = MmioReg::new(0x04);

#[no_mangle]
pub fn main(_env: Environ) {
    let mut folio = MappedFolio::create_pinned(MMIO_BASE, MMIO_SIZE).unwrap();
    let high: u32 = TIME_HIGH_REG.read(&mut folio);
    let low: u32 = TIME_LOW_REG.read(&mut folio);
    let now: u64 = (high as u64) >> 32 | (low as u64);
    info!("now = {now}");
}
