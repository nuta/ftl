BUILD ?= debug
ifeq ($(BUILD),release)
CARGOFLAGS += --release
endif

MAKEFLAGS += --no-builtin-rules --no-builtin-variables
.SUFFIXES:

ifeq ($(V),)
.SILENT:
endif

PROGRESS  ?= printf "  \\033[1;96m%8s\\033[0m  \\033[1;m%s\\033[0m\\n"

NM ?= rust-nm
RUST_GDB ?= riscv64-unknown-elf-gdb
GDB ?= rust-gdb

QEMU ?= $(QEMU_PREFIX)qemu-system-riscv64
QEMUFLAGS += -smp 1 -m 128 -machine virt -bios default
QEMUFLAGS += -nographic -serial mon:stdio
QEMUFLAGS += --no-reboot -d unimp,guest_errors,int,cpu_reset -D qemu-debug.log

CARGO ?= cargo
CARGOFLAGS += -Z build-std=core,alloc -Z build-std-features=compiler-builtins-mem
# RUSTFLAGS += -Z macro-backtrace

ifneq ($(GDBSERVER),)
QEMUFLAGS += -S -gdb tcp::7777
endif

.PHONY: run
run:
	$(PROGRESS) CARGO apps/hello
	RUSTFLAGS="$(RUSTFLAGS)" $(CARGO) build $(CARGOFLAGS) --target libs/user/arch/riscv64/riscv64-qemu-virt.json --manifest-path apps/hello/Cargo.toml
	cp target/riscv64-qemu-virt/$(BUILD)/hello hello.elf

	$(PROGRESS) MKBOOTFS bootfs.bin
	cd devtools && cargo build --release
	cp hello.elf bootfs/startup.elf
	./target/release/devtools mkbootfs bootfs bootfs.bin

	$(PROGRESS) CARGO kernel
	RUSTFLAGS="$(RUSTFLAGS)" $(CARGO) build $(CARGOFLAGS) --target kernel/arch/riscv64/riscv64-qemu-virt.json --manifest-path kernel/Cargo.toml
	cp target/riscv64-qemu-virt/$(BUILD)/kernel ftl.elf

	$(PROGRESS) "NM" kernel.symbols
	$(NM) ftl.elf | rustfilt | awk '{ $$2=""; print $$0 }' > kernel.symbols

	$(PROGRESS) "SYMBOLS" ftl.elf
	python3 embed-symbol-table.py kernel.symbols ftl.elf

	$(PROGRESS) QEMU ftl.elf
	$(QEMU) $(QEMUFLAGS) -kernel ftl.elf

.PHONY: prerequisites
prerequisites:
	$(CARGO) install cargo-binutils rustfilt

.PHONY: rustdoc
rustdoc:
	$(CARGO) doc --workspace

.PHONY: check
check:
	RUSTFLAGS="$(RUSTFLAGS)" $(CARGO) watch -c -- $(CARGO) check $(CARGOFLAGS) --target kernel/arch/riscv64/riscv64-qemu-virt.json --manifest-path kernel/Cargo.toml

.PHONY: test
test:
	$(CARGO) test $(CARGOFLAGS) --manifest-path kernel/Cargo.toml
	$(PROGRESS) QEMU ftl.test.elf
	$(QEMU) $(QEMUFLAGS) -kernel ftl.test.elf


.PHONY: gdb
gdb:
	$(PROGRESS) GDB gdbinit
	RUST_GDB=$(RUST_GDB) $(GDB) -q -ex "source gdbinit"

.PHONY: clean
clean:
	rm -f ftl.elf qemu-debug.log
