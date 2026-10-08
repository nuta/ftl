use ftl_types::error::ErrorCode;

use crate::types::errno::Errno;
use crate::vm::trigger_proactive_page_faults;

/// Fills the buffer with random bytes.
pub fn read(buf: &mut [u8]) -> Result<(), Errno> {
    loop {
        match ftl::random::read(buf) {
            Ok(()) => {
                return Ok(());
            }
            Err(ErrorCode::PageAbsent) => {
                trigger_proactive_page_faults(buf);
            }
            Err(error) => {
                return Err(error.into());
            }
        }
    }
}
