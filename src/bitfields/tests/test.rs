use std::mem::size_of;

use bitfields::*;

#[test]
fn derive() {
    // #[derive(BitEnum)]
    // #[bit_enum(width = 2)]
    // enum TrapMode {
    //     Direct = 0,
    //     Vectored = 1,
    // }

    #[bitfields(u32)]
    struct Stvec {
        // #[bitfield(0..=1)]
        // mode: TrapMode,
        mode: B2,
        // #[bitfield(2..=31)]
        addr: B30,
    }

    assert_eq!(B1::BITS, 1);
    assert_eq!(B2::BITS, 2);

    assert_eq!(size_of::<Stvec>(), size_of::<u32>());
    let mut stvec = Stvec::default();
    assert_eq!(Stvec::mode_offset(), 0);
    assert_eq!(Stvec::addr_offset(), 2);
    assert_eq!(stvec.mode(), 0);
    assert_eq!(stvec.addr(), 0);
    stvec.set_mode(1);
    stvec.set_addr(0x1234567);
    assert_eq!(stvec.mode(), 1);
    assert_eq!(stvec.addr(), 0x1234567);
}
