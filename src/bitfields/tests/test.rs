use std::mem::size_of;

use bitfields::*;

#[test]
fn derive() {
    #[bitfields(bits = 2)]
    #[derive(PartialEq)]
    enum TrapMode {
        Direct = 0b00,
        Vectored = 0b01,
    }

    #[bitfields(u32)]
    struct Stvec {
        #[bitfield(default=TrapMode::Direct)]
        mode: TrapMode,
        // #[bitfield(hidden)]
        // pad: B3,
        // #[bitfield()]
        addr: B30,
    }

    assert_eq!(B1::BITS, 1);
    assert_eq!(B2::BITS, 2);

    assert_eq!(size_of::<Stvec>(), size_of::<u32>());
    let mut stvec = Stvec::default();
    assert_eq!(Stvec::mode_offset(), 0);
    assert_eq!(Stvec::addr_offset(), 2);
    assert_eq!(stvec.mode(), TrapMode::Direct);
    assert_eq!(stvec.addr(), 0);
    stvec.set_mode(TrapMode::Vectored);
    stvec.set_addr(0x1234567);
    assert_eq!(stvec.mode(), TrapMode::Vectored);
    assert_eq!(stvec.addr(), 0x1234567);
}
