use std::fmt;

pub struct Color {
    text: &'static str,
}

impl Color {
    #[allow(unused)]
    pub const fn red() -> Color {
        Color { text: "\x1b[31m" }
    }

    #[allow(unused)]
    pub const fn green() -> Color {
        Color { text: "\x1b[32m" }
    }

    #[allow(unused)]
    pub const fn yellow() -> Color {
        Color { text: "\x1b[33m" }
    }

    #[allow(unused)]
    pub const fn blue() -> Color {
        Color { text: "\x1b[34m" }
    }

    #[allow(unused)]
    pub const fn magenta() -> Color {
        Color { text: "\x1b[35m" }
    }

    #[allow(unused)]
    pub const fn cyan() -> Color {
        Color { text: "\x1b[36m" }
    }

    #[allow(unused)]
    pub const fn bold() -> Color {
        Color { text: "\x1b[1m" }
    }

    #[allow(unused)]
    pub const fn reset_all() -> Color {
        Color { text: "\x1b[0m" }
    }

    #[allow(unused)]
    pub const fn reset_fg() -> Color {
        Color { text: "\x1b[39m" }
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.text)
    }
}
