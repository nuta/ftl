use bit_types::{b1, BitStruct};

#[test]
fn derive() {
    enum TrapMode {
        Direct = 0,
        Vectored = 1,
    }

    #[derive(BitStruct)]
    struct Stvec {
        #[bit_types(offset = 0, width = 2)]
        mode: b1,
        #[bit_types(offset = 2, width = 62)]
        addr: u32,
    }
}
