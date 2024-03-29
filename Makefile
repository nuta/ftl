ARCH    ?= riscv64
MACHINE ?= qemu-virt
RELEASE ?=            # "1" to build release version
V       ?=            # "1" to enable verbose output
GDB	    ?=            # "1" to enable GDB debugging

# The default build target.
.PHONY: default
default: ftl.elf

# Disable builtin implicit rules and variables.
MAKEFLAGS += --no-builtin-rules --no-builtin-variables
.SUFFIXES:

# Enable verbose output if $(V) is set.
ifeq ($(V),)
.SILENT:
endif

ifeq ($(RELEASE),1)
BUILD := release
CARGOFLAGS += --release
else
BUILD := debug
endif

QEMU     ?= qemu-system-riscv64
RUST_GDB ?= rust-gdb
CARGO    := cargo
PROGRESS := printf "  \\033[1;96m%8s\\033[0m  \\033[1;m%s\\033[0m\\n"

CARGOFLAGS += -Z build-std=core,alloc -Z build-std-features=compiler-builtins-mem
CARGOFLAGS += --target boot/$(ARCH)/$(ARCH)-$(MACHINE).json

QEMUFLAGS += -machine virt -m 256 -bios default
QEMUFLAGS += -nographic -serial mon:stdio --no-reboot
QEMUFLAGS += -d cpu_reset,unimp,guest_errors,int -D qemu.log
QEMUFLAGS += $(if $(GDB),-gdb tcp::7789 -S)

sources += \
    $(shell find boot/$(ARCH) ftl -name '*.rs') \
    $(shell find boot/$(ARCH) ftl -name '*.S')

.PHONY: run
run: ftl.elf
	$(PROGRESS) "QEMU" "ftl.elf"
	$(QEMU) $(QEMUFLAGS) -kernel ftl.elf | tee kernel.log

.PHONY: gdb
gdb:
	$(PROGRESS) "GDB" "ftl.elf"
	RUST_GDB=riscv64-elf-gdb $(RUST_GDB) -q

ftl.elf: $(sources) Makefile
	$(PROGRESS) "CARGO" "boot/$(ARCH)"
	$(CARGO) build $(CARGOFLAGS) --manifest-path boot/$(ARCH)/Cargo.toml
	cp target/$(ARCH)-$(MACHINE)/$(BUILD)/boot_$(ARCH) ftl.elf
