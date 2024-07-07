# "riscv64" or "arm64"
ARCH    ?= arm64

MACHINE ?= qemu-virt
RELEASE ?=            # "1" to build release version
V       ?=            # "1" to enable verbose output
STARTUP ?= apps/hello

# Note: Don't forget to update boot.spec.json as well!
APPS    ?= apps/ping apps/pong apps/virtio_blk

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

ifeq ($(ARCH),riscv64)
QEMU      ?= qemu-system-riscv64
QEMUFLAGS += -machine virt -m 256 -bios default
QEMUFLAGS += -global virtio-mmio.force-legacy=false
QEMUFLAGS += -drive id=drive0,file=disk.img,format=raw,if=none
QEMUFLAGS += -device virtio-blk-device,drive=drive0,bus=virtio-mmio-bus.0
else ifeq ($(ARCH),arm64)
QEMU      ?= qemu-system-aarch64
QEMUFLAGS += -m 512
QEMUFLAGS += -machine virt $(if $(KVM), -accel kvm -cpu host, -cpu cortex-a76)
QEMUFLAGS += -global virtio-mmio.force-legacy=false
QEMUFLAGS += -drive id=drive0,file=disk.img,format=raw,if=none
QEMUFLAGS += -device virtio-blk-device,drive=drive0,bus=virtio-mmio-bus.0
else
$(error "Unknown ARCH: $(ARCH)")
endif

CARGO    ?= cargo
PROGRESS ?= printf "  \\033[1;96m%8s\\033[0m  \\033[1;m%s\\033[0m\\n"
OBJCOPY  ?= llvm-objcopy

RUSTFLAGS += -Z macro-backtrace --emit asm
CARGOFLAGS += -Z build-std=core,alloc -Z build-std-features=compiler-builtins-mem

export CARGO_TERM_HYPERLINKS=false

QEMUFLAGS += -nographic -serial mon:stdio --no-reboot
QEMUFLAGS += -d cpu_reset,unimp,guest_errors,int -D qemu.log
QEMUFLAGS += $(if $(GDB),-gdb tcp::7789 -S)

app_elfs := $(foreach app,$(APPS),build/$(app).elf)
sources += \
	$(shell find \
		boot/$(ARCH) kernel libs apps \
		-name '*.rs' -o -name '*.json' -o -name '*.ld' -o -name '*.toml' -o -name '*.S' \
	)

.DEFAULT_GOAL := default
default: ftl.elf

.PHONY: run
run: ftl.elf disk.img
	$(PROGRESS) "QEMU" "ftl.elf"
	$(QEMU) $(QEMUFLAGS) -kernel ftl.elf

.PHONY: clean
clean:
	rm -rf build

.PHONY: clippy
clippy:
	RUSTFLAGS="$(RUSTFLAGS)" $(CARGO) clippy --fix --allow-dirty --allow-staged $(CARGOFLAGS) --manifest-path boot/$(ARCH)/Cargo.toml

.PHONY: fmt
fmt:
	find boot kernel libs apps tools -name '*.rs' | xargs rustup run nightly rustfmt

.PHONY: fix
fix:
	cargo clippy --fix --allow-dirty --allow-staged $(CARGOFLAGS)

disk.img:
	$(PROGRESS) "GEN" "$(@)"
	dd if=/dev/zero of=$(@) bs=1M count=8

ftl.elf: $(sources) libs/rust/ftl_autogen/lib.rs Makefile build/bootfs.bin
	$(PROGRESS) "CARGO" "boot/$(ARCH)"
	RUSTFLAGS="$(RUSTFLAGS)" CARGO_TARGET_DIR="build/cargo" $(CARGO) build $(CARGOFLAGS) \
		--target boot/$(ARCH)/$(ARCH)-$(MACHINE).json \
		--manifest-path boot/$(ARCH)/Cargo.toml
	cp build/cargo/$(ARCH)-$(MACHINE)/$(BUILD)/boot_$(ARCH) $(@)

build/startup.elf: build/$(STARTUP).elf
	cp $< $@

build/bootfs.bin: build/ftl_mkbootfs $(app_elfs) Makefile
	rm -rf build/bootfs
	mkdir -p build/bootfs
	mkdir -p build/bootfs/cfg
	cp boot.spec.json build/bootfs/cfg/boot.spec.json
	for app_elf in $(app_elfs); do \
		app_name=$$(basename $${app_elf%.elf}); \
		mkdir -p build/bootfs/apps/$${app_name}; \
		cp $${app_elf}.stripped build/bootfs/apps/$${app_name}/app.elf; \
		cp apps/$${app_name}/app.spec.json build/bootfs/apps/$${app_name}/app.spec.json; \
	done
	$(PROGRESS) "MKBOOTFS" "$(@)"
	./build/ftl_mkbootfs -o $(@) build/bootfs

# TODO: Can't add "-C link-args=-Map=$(@:.elf=.map)" to RUSTFLAGS because rustc considers it as
#       a change in compiler flags. Indeed it is, but it doesn't affect the output binary.
#
#       I'll file an issue on rust-lang/rust to hear  community's opinion.
build/%.elf: $(sources) libs/rust/ftl_autogen/lib.rs Makefile
	$(PROGRESS) "CARGO" "$(@)"
	mkdir -p $(@D)
	RUSTFLAGS="$(RUSTFLAGS)" \
	CARGO_TARGET_DIR="build/cargo" \
		$(CARGO) build $(CARGOFLAGS) \
		--target libs/rust/ftl_api/arch/$(ARCH)/$(ARCH)-user.json \
		--manifest-path $(patsubst build/%.elf,%,$(@))/Cargo.toml
	cp build/cargo/$(ARCH)-user/$(BUILD)/$(patsubst build/apps/%.elf,%,$(@)) $(@)
	$(OBJCOPY) --strip-all --strip-debug $(@) $(@).stripped

build/ftl_idlc: $(shell find tools/idlc libs/rust/ftl_types -name '*.rs') $(shell find tools/idlc -name '*.j2')
	mkdir -p $(@D)
	$(PROGRESS) "CARGO" "tools/idlc"
	RUSTFLAGS="$(RUSTFLAGS)" \
	CARGO_TARGET_DIR="build/cargo" \
		$(CARGO) build \
			$(if $(RELEASE),--release,) \
			--manifest-path tools/idlc/Cargo.toml
	mv build/cargo/$(BUILD)/ftl_idlc $(@)

build/ftl_mkbootfs: $(shell find tools/mkbootfs -name '*.rs')
	mkdir -p $(@D)
	$(PROGRESS) "CARGO" "tools/mkbootfs"
	RUSTFLAGS="$(RUSTFLAGS)" \
	CARGO_TARGET_DIR="build/cargo" \
		$(CARGO) build \
			$(if $(RELEASE),--release,) \
			--manifest-path tools/mkbootfs/Cargo.toml
	mv build/cargo/$(BUILD)/ftl_mkbootfs $(@)

libs/rust/ftl_autogen/lib.rs: idl.json build/ftl_idlc $(shell find $(APPS) -name '*.spec.json') Makefile
	mkdir -p build
	$(PROGRESS) "ILDC" "$(@)"
	./build/ftl_idlc \
		--autogen-outfile $(@) \
		--api-autogen-outfile libs/rust/ftl_api_autogen/lib.rs \
		--idl-file idl.json \
		$(foreach app_dir,$(APPS),--app-specs $(app_dir)/app.spec.json)
