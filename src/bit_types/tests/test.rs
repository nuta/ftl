use std::mem::size_of;

use bit_types::*;

#[test]
fn derive() {
    // #[derive(BitEnum)]
    // #[bit_enum(width = 2)]
    // enum TrapMode {
    //     Direct = 0,
    //     Vectored = 1,
    // }

    #[bit_struct(width = 32)]
    struct Stvec {
        #[field(0..=1)]
        // mode: TrapMode,
        mode: b2,
        #[field(2..=31)]
        addr: b30,
    }

    assert_eq!(size_of::<Stvec>(), size_of::<u32>());
}
