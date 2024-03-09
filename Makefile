ARCH    ?= riscv64
MACHINE ?= qemu-virt
RELEASE ?=            # "1" to build release version
V       ?=            # "1" to enable verbose output
GDB	    ?=            # "1" to enable GDB debugging
INKERNEL_FIBERS ?= riscv_plic virtio_net

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

RUSTFLAGS += -Z macro-backtrace
CARGOFLAGS += -Z build-std=core,alloc -Z build-std-features=compiler-builtins-mem
CARGOFLAGS += --target boot/$(ARCH)/$(ARCH)-$(MACHINE).json

QEMUFLAGS += -machine virt -m 256 -bios default
QEMUFLAGS += -nographic -serial mon:stdio --no-reboot
QEMUFLAGS += -d cpu_reset,unimp,guest_errors,int -D qemu.log
QEMUFLAGS += -netdev user,id=net0
QEMUFLAGS += -global virtio-mmio.force-legacy=false
QEMUFLAGS += -device virtio-net-device,netdev=net0
QEMUFLAGS += -object filter-dump,id=fiter0,netdev=net0,file=virtio-net.pcap
QEMUFLAGS += $(if $(GDB),-gdb tcp::7789 -S)

sources += \
    $(shell find boot/$(ARCH) kernel api libs fibers devtools -name '*.rs') \
    $(shell find boot/$(ARCH) kernel api libs fibers devtools -name '*.yaml') \
    $(shell find boot/$(ARCH) kernel api libs fibers devtools -name '*.S')

.PHONY: run
run: ftl.elf
	$(PROGRESS) "QEMU" "ftl.elf"
	$(QEMU) $(QEMUFLAGS) -kernel ftl.elf

.PHONY: clean
clean:
	rm -f build

.PHONY: doc
doc:
	$(PROGRESS) "CARGO" "doc"
	$(CARGO) doc $(CARGOFLAGS) --manifest-path api/Cargo.toml
	$(CARGO) doc $(CARGOFLAGS) --manifest-path kernel_api/Cargo.toml

.PHONY: gdb
gdb:
	$(PROGRESS) "GDB" "ftl.elf"
	RUST_GDB=riscv64-elf-gdb $(RUST_GDB) -q

ftl.elf: $(sources) Makefile
	$(PROGRESS) "DEVTOOLS" "autogen"
	cargo run --quiet --release --bin ftl autogen --outdir libs $(foreach fiber,$(INKERNEL_FIBERS), fibers/$(fiber))
	$(PROGRESS) "CARGO" "boot/$(ARCH)"
	RUSTFLAGS="$(RUSTFLAGS)" CARGO_TARGET_DIR="build/cargo" $(CARGO) build $(CARGOFLAGS) --manifest-path boot/$(ARCH)/Cargo.toml
	cp build/cargo/$(ARCH)-$(MACHINE)/$(BUILD)/boot_$(ARCH) $(@)

clippy:
	RUSTFLAGS="$(RUSTFLAGS)" $(CARGO) clippy --fix --allow-dirty --allow-staged $(CARGOFLAGS) --manifest-path boot/$(ARCH)/Cargo.toml
