use core::arch::asm;
use core::cell::RefMut;
use core::mem::offset_of;

use super::gdt::KERNEL_CS;
use super::thread::Context;
use crate::refcount::SharedRef;
use crate::thread::Thread;

#[naked]
pub unsafe extern "C" fn kernel_syscall_entry(
    _a0: isize,
    _a1: isize,
    _a2: isize,
    _a3: isize,
    _a4: isize,
    _a5: isize,
) -> isize {
    asm!(
        r#"
            mov rax, gs:[{context_offset}]

            // Save general-purpose registers.
            mov [rax + {rbx_offset}], rbx
            mov [rax + {rcx_offset}], rcx
            mov [rax + {rdx_offset}], rdx
            mov [rax + {rsi_offset}], rsi
            mov [rax + {rdi_offset}], rdi
            mov [rax + {rbp_offset}], rbp
            mov [rax + {r8_offset}],  r8
            mov [rax + {r9_offset}],  r9
            mov [rax + {r10_offset}], r10
            mov [rax + {r11_offset}], r11
            mov [rax + {r12_offset}], r12
            mov [rax + {r13_offset}], r13
            mov [rax + {r14_offset}], r14
            mov [rax + {r15_offset}], r15

            mov rbx, rax
            pop rax
            mov [rbx + {rip_offset}], rax
            mov [rbx + {rsp_offset}], rsp

            // Handle the system call.
            call {syscall_handler}

            // Save the return value in the thread context, and switch
            // to the next thread.
            push rbx
            mov rbx, gs:[{context_offset}]
            mov [rbx + {rax_offset}], rax
            pop rbx

            jmp {switch_thread}
        "#,
        context_offset = const offset_of!(crate::arch::CpuVar, context),
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
        syscall_handler = sym crate::syscall::syscall_handler,
        switch_thread = sym crate::thread::switch_thread,
        options(noreturn)
    )
}

pub fn return_to_user(thread: *mut super::Thread, sysret: Option<isize>) -> ! {
    let context: *mut Context = unsafe { &mut (*thread).context as *mut _ };
    if let Some(value) = sysret {
        unsafe {
            (*context).rax = value as usize;
        }
    }

    unsafe {
        asm!(
            r#"
                mov gs:[{context_offset}], rax

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
                push 0                    // SS
                push [rax + {rsp_offset}] // RSP
                push 0x002                // RFLAGS
                push {iret_cs}            // CS
                push [rax + {rip_offset}] // RIP

                // Restore RAX
                mov rax, [rax + {rax_offset}]

                // Restore RIP/RSP and resume the thread execution.
                iretq
            "#,
            in ("rax") context as usize,
            iret_cs = const KERNEL_CS,
            context_offset = const offset_of!(crate::arch::CpuVar, context),
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
