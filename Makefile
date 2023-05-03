MAKEFLAGS += --no-builtin-rules --no-builtin-variables
.SUFFIXES:

ifeq ($(V),)
.SILENT:
endif

PROGRESS  ?= printf "  \\033[1;96m%8s\\033[0m  \\033[1;m%s\\033[0m\\n"

QEMU ?= $(QEMU_PREFIX)qemu-system-riscv32
QEMUFLAGS += -smp 1 -m 128 -machine virt,aclint=on -bios none
QEMUFLAGS += -nographic -serial mon:stdio
QEMUFLAGS += --no-reboot -d unimp,guest_errors,int,cpu_reset -D qemu-debug.log

CARGO ?= cargo
CARGOFLAGS += -Z build-std=core -Z build-std-features=compiler-builtins-mem
CARGOFLAGS += --target src/boot2rust/riscv32-qemu-virt.json

LD := $(LLVM_PREFIX)ld.lld
LDFLAGS = -Tsrc/boot2rust/riscv32-qemu-virt.ld

.PHONY: run
run:
	$(MAKE) ftl.elf
	$(PROGRESS) QEMU ftl.elf
	$(QEMU) $(QEMUFLAGS) -kernel ftl.elf

.PHONY: clean
clean:
	rm -f ftl.elf qemu-debug.log

ftl.elf: $(wildcard src/*/*) $(wildcard src/*/*/*) Cargo.toml Makefile
	$(PROGRESS) CARGO $@
	$(CARGO) build $(CARGOFLAGS) --manifest-path src/boot2rust/Cargo.toml
	$(PROGRESS) LD $@
	$(LD) $(LDFLAGS) -o ftl.elf target/riscv32-qemu-virt/debug/libboot2rust.a
