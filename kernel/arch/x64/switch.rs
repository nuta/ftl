use core::arch::asm;
use core::cell::RefMut;
use core::mem::offset_of;

use super::thread::Context;
use crate::refcount::SharedRef;
use crate::thread::Thread;

pub fn return_to_user(current_thread: RefMut<'_, SharedRef<Thread>>, sysret: Option<isize>) -> ! {
    let context: *mut Context = &current_thread.arch().context as *const _ as *mut _;
    if let Some(value) = sysret {
        unsafe {
            (*context).rax = value as usize;
        }
    }

    drop(current_thread);
    println!("Returning to user");

    unsafe {
        asm!(
            r#"
                // Restore general-purpose registers except RAX/RSP.
                mov rbx, [rax + {rbx_offset}]
                mov rcx, [rax + {rcx_offset}]
                mov rdx, [rax + {rdx_offset}]
                mov rsi, [rax + {rsi_offset}]
                mov rdi, [rax + {rdi_offset}]
                mov rbp, [rax + {rbp_offset}]
                mov r8,  [rax + {r8_offset}]
                mov r9,  [rax + {r9_offset}]
                mov r10, [rax + {r10_offset}]
                mov r11, [rax + {r11_offset}]
                mov r12, [rax + {r12_offset}]
                mov r13, [rax + {r13_offset}]
                mov r14, [rax + {r14_offset}]
                mov r15, [rax + {r15_offset}]

                // Build an IRET frame
                push [rax + {rip_offset}] // RIP
                push 8                 // CS (FIXME:)
                push 0x202             // RFLAGS
                push [rax + {rsp_offset}] // RSP
                push 0                 // SS (FIXME:)

                // Restore RAX
                mov rax, [rax + {rax_offset}]

                // Restore RIP/RSP and resume the thread execution.
                iretq
            "#,
            in ("rax") context as usize,
            rip_offset = const offset_of!(Context, rip),
            rax_offset = const offset_of!(Context, rax),
            rbx_offset = const offset_of!(Context, rbx),
            rcx_offset = const offset_of!(Context, rcx),
            rdx_offset = const offset_of!(Context, rdx),
            rsi_offset = const offset_of!(Context, rsi),
            rdi_offset = const offset_of!(Context, rdi),
            rbp_offset = const offset_of!(Context, rbp),
            rsp_offset = const offset_of!(Context, rsp),
            r8_offset = const offset_of!(Context, r8),
            r9_offset = const offset_of!(Context, r9),
            r10_offset = const offset_of!(Context, r10),
            r11_offset = const offset_of!(Context, r11),
            r12_offset = const offset_of!(Context, r12),
            r13_offset = const offset_of!(Context, r13),
            r14_offset = const offset_of!(Context, r14),
            r15_offset = const offset_of!(Context, r15),
            options(noreturn)
        );
    }
}
