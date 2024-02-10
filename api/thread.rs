use crate::syscall;

pub fn yield_cpu() {
    syscall::yield_cpu();
}
