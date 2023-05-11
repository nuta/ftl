use std::{
    fmt::{self, Debug},
    mem::size_of,
};

use bitfields::*;

#[test]
fn derive() {
    #[derive(PartialEq)]
    #[bitfields(bits = 2)]
    enum TrapMode {
        Direct = 0b00,
        Vectored = 0b01,
    }

    impl Debug for TrapMode {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                TrapMode::Direct => write!(f, "Direct"),
                TrapMode::Vectored => write!(f, "Vectored"),
                _ => write!(f, "Unknown"),
            }
        }
    }

    #[bitfields(u32)]
    struct Stvec {
        // #[bitfield()]
        mode: TrapMode,
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
