//! KVM paravirtualized clock (kvmclock).
//!
//! <https://docs.kernel.org/virt/kvm/x86/msr.html>
use core::arch::x86_64::__cpuid;
use core::mem::offset_of;
use core::ptr;
use core::sync::atomic::AtomicBool;
use core::sync::atomic::AtomicI8;
use core::sync::atomic::AtomicU8;
use core::sync::atomic::AtomicU32;
use core::sync::atomic::AtomicU64;
use core::sync::atomic::Ordering;
use core::sync::atomic::fence;

use ftl_types::time::WallTime;

use super::msr::wrmsr;
use super::timer::read_tsc;
use super::vmspace::vaddr2paddr;
use crate::address::VAddr;

const MSR_KVM_WALL_CLOCK_NEW: u32 = 0x4b56_4d00;
const MSR_KVM_SYSTEM_TIME_NEW: u32 = 0x4b56_4d01;
const SYSTEM_TIME_ENABLE: u64 = 1 << 0;

/// `struct pvclock_wall_clock`.
#[repr(C)]
struct PvWallClock {
    version: AtomicU32,
    sec: AtomicU32,
    nsec: AtomicU32,
}

/// `struct pvclock_vcpu_time_info`.
#[repr(C)]
struct VCpuTimeInfo {
    version: AtomicU32,
    pad0: AtomicU32,
    tsc_timestamp: AtomicU64,
    system_time: AtomicU64,
    tsc_to_system_mul: AtomicU32,
    tsc_shift: AtomicI8,
    flags: AtomicU8,
    pad: [AtomicU8; 2],
}

impl PvWallClock {
    const fn zeroed() -> Self {
        Self {
            version: AtomicU32::new(0),
            sec: AtomicU32::new(0),
            nsec: AtomicU32::new(0),
        }
    }

    fn read_nanos(&self) -> u64 {
        read_consistent(&self.version, || {
            let sec = self.sec.load(Ordering::Relaxed);
            let nsec = self.nsec.load(Ordering::Relaxed);
            u64::from(sec) * 1_000_000_000 + u64::from(nsec)
        })
    }
}

impl VCpuTimeInfo {
    const fn zeroed() -> Self {
        Self {
            version: AtomicU32::new(0),
            pad0: AtomicU32::new(0),
            tsc_timestamp: AtomicU64::new(0),
            system_time: AtomicU64::new(0),
            tsc_to_system_mul: AtomicU32::new(0),
            tsc_shift: AtomicI8::new(0),
            flags: AtomicU8::new(0),
            pad: [AtomicU8::new(0), AtomicU8::new(0)],
        }
    }

    fn read_nanos(&self) -> u64 {
        read_consistent(&self.version, || {
            let tsc_timestamp = self.tsc_timestamp.load(Ordering::Relaxed);
            let system_time = self.system_time.load(Ordering::Relaxed);
            let mul = self.tsc_to_system_mul.load(Ordering::Relaxed);
            let shift = self.tsc_shift.load(Ordering::Relaxed);

            let mut delta = read_tsc().wrapping_sub(tsc_timestamp);
            if shift >= 0 {
                delta <<= shift;
            } else {
                delta >>= -shift;
            }

            let scaled = (u128::from(delta) * u128::from(mul)) >> 32;
            system_time.wrapping_add(scaled as u64)
        })
    }
}

fn read_consistent<T>(version: &AtomicU32, read: impl Fn() -> T) -> T {
    loop {
        let before = version.load(Ordering::Acquire);
        let value = read();
        fence(Ordering::Acquire);
        let after = version.load(Ordering::Relaxed);
        if before & 1 == 0 && before == after {
            return value;
        }

        core::hint::spin_loop();
    }
}

fn is_supported() -> bool {
    // Is this running in a hypervisor guest?
    if __cpuid(1).ecx & (1 << 31) == 0 {
        return false;
    }

    // Check if it is in KVM.
    let leaf = __cpuid(0x4000_0000);
    let signature = [
        leaf.ebx.to_le_bytes(),
        leaf.ecx.to_le_bytes(),
        leaf.edx.to_le_bytes(),
    ];
    if signature.as_flattened() != b"KVMKVMKVM\0\0\0" {
        return false;
    }

    // Does KVM provide kvmclock?
    __cpuid(0x4000_0001).eax & (1 << 3) != 0
}

#[repr(C, align(4096))]
struct ClockPage {
    vcpu_time_info: VCpuTimeInfo,
    wall_clock: PvWallClock,
}

static ENABLED: AtomicBool = AtomicBool::new(false);

static CLOCK_PAGE: ClockPage = ClockPage {
    vcpu_time_info: VCpuTimeInfo::zeroed(),
    wall_clock: PvWallClock::zeroed(),
};

pub(super) fn is_enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

pub(super) fn system_time_read() -> u64 {
    CLOCK_PAGE.vcpu_time_info.read_nanos()
}

fn clock_page_paddr() -> usize {
    let vaddr = VAddr::new(ptr::from_ref(&CLOCK_PAGE).addr());
    let paddr = vaddr2paddr(vaddr);
    paddr.as_usize()
}

pub(super) fn walltime_read() -> WallTime {
    let paddr = clock_page_paddr() + offset_of!(ClockPage, wall_clock);
    unsafe {
        wrmsr(MSR_KVM_WALL_CLOCK_NEW, paddr as u64);
    }

    let boot_nanos = CLOCK_PAGE.wall_clock.read_nanos();
    WallTime::from_nanos(boot_nanos.wrapping_add(system_time_read()))
}

pub(super) fn init() -> bool {
    if !is_supported() {
        return false;
    }

    let paddr = clock_page_paddr() + offset_of!(ClockPage, vcpu_time_info);
    unsafe {
        wrmsr(MSR_KVM_SYSTEM_TIME_NEW, (paddr as u64) | SYSTEM_TIME_ENABLE);
    }

    ENABLED.store(true, Ordering::Relaxed);
    true
}
