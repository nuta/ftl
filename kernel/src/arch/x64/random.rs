use core::arch::asm;

// TODO: Check if RDRAND is supported.
fn rdrand64() -> u64 {
    loop {
        let mut value: u64;
        let mut ok: u8;
        unsafe {
            asm!(
                "rdrand {value}", // CF=1 if RDRAND succeeded
                "setc {ok}",      // Read CF
                value = out(reg) value,
                ok = out(reg_byte) ok,
                options(nomem, nostack),
            );
        }

        if ok != 0 {
            return value;
        }

        core::hint::spin_loop();
    }
}

pub fn random_read(buf: &mut [u8]) {
    for chunk in buf.chunks_mut(8) {
        let bytes = rdrand64().to_ne_bytes();
        chunk.copy_from_slice(&bytes[..chunk.len()]);
    }
}
