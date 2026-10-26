use alloc::vec::Vec;
use core::mem;

use crate::types::errno::Errno;

const LINE_LEN_MAX: usize = 4096;

/// A zone spec. What you write to `/lx/zone/fork`.
pub struct Spec {
    pub net: bool,
    pub init: Vec<u8>,
}

/// A parser for zone specs.
pub struct SpecParser {
    net: bool,
    init: Option<Vec<u8>>,
    line: Vec<u8>,
}

impl SpecParser {
    pub const fn new() -> Self {
        Self {
            net: false,
            init: None,
            line: Vec::new(),
        }
    }

    pub fn feed(&mut self, buf: &[u8]) -> Result<Option<Spec>, Errno> {
        for &b in buf {
            if b == b'\n' {
                let line = mem::take(&mut self.line);
                match self.feed_line(&line) {
                    Ok(Some(spec)) => return Ok(Some(spec)),
                    Ok(None) => continue,
                    Err(err) => return Err(err),
                }
            }

            if self.line.len() >= LINE_LEN_MAX {
                return Err(Errno::E2BIG);
            }

            self.line.push(b);
        }

        Ok(None)
    }

    fn feed_line(&mut self, line: &[u8]) -> Result<Option<Spec>, Errno> {
        // TODO: Use split_once once we it gets stabilized.
        let mut parts = line.splitn(2, |b| *b == b' ');
        let key = parts.next().unwrap_or(b"");
        let value = parts.next().unwrap_or(b"");

        match key {
            b"net" => {
                if !value.is_empty() {
                    return Err(Errno::EINVAL);
                }

                if mem::replace(&mut self.net, true) {
                    // Multiple net specs are not allowed.
                    return Err(Errno::EINVAL);
                }

                Ok(None)
            }
            b"init" => {
                if value.is_empty() || !value.iter().all(|b| b.is_ascii_graphic()) {
                    return Err(Errno::EINVAL);
                }

                if self.init.replace(value.to_vec()).is_some() {
                    // Multiple init specs are not allowed.
                    return Err(Errno::EINVAL);
                }

                Ok(None)
            }
            b"exec" => {
                if !value.is_empty() {
                    return Err(Errno::EINVAL);
                }

                let init = self.init.take().ok_or(Errno::EINVAL)?;
                Ok(Some(Spec {
                    net: self.net,
                    init,
                }))
            }
            _ => Err(Errno::EINVAL),
        }
    }
}
