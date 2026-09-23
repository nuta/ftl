#!/bin/bash
set -eu -o pipefail

RELEASE=${RELEASE:-}
ARCH=${ARCH:-x64}

build_rust_app() {
  local name="$1"
  pushd "apps/${name}"
  cargo build --release --target x86_64-unknown-linux-musl
  popd
  cp "apps/${name}/target/x86_64-unknown-linux-musl/release/${name}" "initfs/bin/${name}"
}

build_default_initfs() {
  mkdir -p initfs/bin
  build_rust_app echo
  build_rust_app httpd
}

build_initfs() {
  local initfs_dir="$1"
  local initfs_cpio="$2"

  pushd "${initfs_dir}"
  find * -print0 | cpio -o -0 -H newc > "${initfs_cpio}"
  popd
}

build_os() {
  export CARGO_TERM_HYPERLINKS=false
  CARGOFLAGS=(
      -Z build-std=core,alloc
      -Z build-std-features=compiler-builtins-mem
      -Z json-target-spec
  )

  if [[ -n "${RELEASE:-}" ]]; then
      CARGOFLAGS+=(--release)
      target="release"
  else
      target="debug"
  fi

  cargo_command=build
  if [[ -n "${CHECK:-}" ]]; then
    cargo_command=check
  fi

  # Build userspace OS.
  cargo "${cargo_command}" "${CARGOFLAGS[@]}" --target libs/ftl/src/arch/$ARCH/user.json \
    --manifest-path lx/Cargo.toml

  # Build kernel.
  cargo "${cargo_command}" "${CARGOFLAGS[@]}" --target kernel/src/arch/$ARCH/kernel.json \
    --manifest-path kernel/Cargo.toml

  if [[ "$cargo_command" != "check" ]]; then
    cp target/user/$target/lx lx.elf
    cp target/kernel/$target/kernel ftl.elf
  fi
}

build_iso() {
  if [[ "$ARCH" != "x64" ]]; then
    echo "ISO is only supported for x64"
    exit 1
  fi

  echo "Building ISO..."
  mkdir -p isofiles/boot/grub
  cp kernel/src/arch/x64/grub.cfg isofiles/boot/grub/
  cp ftl.elf lx.elf isofiles/

  export PATH="$PATH:/opt/homebrew/opt/i686-elf-grub/bin"
  i686-elf-grub-mkrescue -o ftl.iso isofiles
}

main() {
  if [[ ! -n "${INITFS:-}" ]]; then
    build_default_initfs
  fi

  build_initfs "${INITFS:-initfs}" "$(pwd)/initfs.cpio"
  build_os

  # Build ISO if $ISO is set.
  if [[ -n "${ISO:-}" ]]; then
    build_iso
  fi
}

main
