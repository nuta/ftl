use crate::types::c_ulong;

pub const TCGETS: c_ulong = 0x5401;
pub const TCSETS: c_ulong = 0x5402;
pub const TCSETSW: c_ulong = 0x5403;
pub const TCSETSF: c_ulong = 0x5404;
pub const TIOCGWINSZ: c_ulong = 0x5413;

pub const ICRNL: u32 = 0o400;
pub const IXON: u32 = 0o2000;

pub const OPOST: u32 = 0o1;
pub const ONLCR: u32 = 0o4;

pub const B38400: u32 = 0o17;
pub const CS8: u32 = 0o60;
pub const CREAD: u32 = 0o200;
pub const HUPCL: u32 = 0o2000;

pub const ISIG: u32 = 0o1;
pub const ICANON: u32 = 0o2;
pub const ECHO: u32 = 0o10;
pub const ECHOE: u32 = 0o20;
pub const ECHOK: u32 = 0o40;
pub const ECHOCTL: u32 = 0o1000;
pub const ECHOKE: u32 = 0o4000;
pub const IEXTEN: u32 = 0o100000;

const NCCS: usize = 19;
const VINTR: usize = 0;
const VQUIT: usize = 1;
const VERASE: usize = 2;
const VKILL: usize = 3;
const VEOF: usize = 4;
const VMIN: usize = 6;
const VSTART: usize = 8;
const VSTOP: usize = 9;
const VSUSP: usize = 10;
const VREPRINT: usize = 12;
const VDISCARD: usize = 13;
const VWERASE: usize = 14;
const VLNEXT: usize = 15;

const fn ctrl(c: u8) -> u8 {
    c & 0x1f
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Termios {
    pub c_iflag: u32,
    pub c_oflag: u32,
    pub c_cflag: u32,
    pub c_lflag: u32,
    pub c_line: u8,
    pub c_cc: [u8; NCCS],
}

impl Termios {
    pub const fn new() -> Self {
        let mut c_cc = [0; NCCS];
        c_cc[VINTR] = ctrl(b'C');
        c_cc[VQUIT] = ctrl(b'\\');
        c_cc[VERASE] = 0x7f; // DEL
        c_cc[VKILL] = ctrl(b'U');
        c_cc[VEOF] = ctrl(b'D');
        c_cc[VMIN] = 1;
        c_cc[VSTART] = ctrl(b'Q');
        c_cc[VSTOP] = ctrl(b'S');
        c_cc[VSUSP] = ctrl(b'Z');
        c_cc[VREPRINT] = ctrl(b'R');
        c_cc[VDISCARD] = ctrl(b'O');
        c_cc[VWERASE] = ctrl(b'W');
        c_cc[VLNEXT] = ctrl(b'V');

        Self {
            c_iflag: ICRNL | IXON,
            c_oflag: OPOST | ONLCR,
            c_cflag: B38400 | CS8 | CREAD | HUPCL,
            c_lflag: ISIG | ICANON | ECHO | ECHOE | ECHOK | ECHOCTL | ECHOKE | IEXTEN,
            c_line: 0,
            c_cc,
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct WinSize {
    pub ws_row: u16,
    pub ws_col: u16,
    pub ws_xpixel: u16,
    pub ws_ypixel: u16,
}
