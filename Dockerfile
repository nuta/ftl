FROM ubuntu:24.04
ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update && apt-get install -y \
    curl \
    qemu-system-riscv64 \
    build-essential

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"
RUN rustup toolchain install nightly
RUN rustup default nightly
RUN rustup target add riscv64gc-unknown-none-elf
RUN rustup component add rust-src llvm-tools

COPY . /os
WORKDIR /os

RUN make ARCH=riscv64 MACHINE=qemu-virt
EXPOSE 1234

CMD [ "qemu-system-riscv64", \
    "-kernel", "ftl.elf", \
    "-nographic", "-serial", "mon:stdio", "--no-reboot", \
    "-machine", "virt", "-m", "256", "-bios", "default", \
    "-global", "virtio-mmio.force-legacy=false", \
    "-device", "virtio-net-device,netdev=net0,bus=virtio-mmio-bus.0", \
    "-netdev", "user,id=net0,hostfwd=tcp:0.0.0.0:1234-:80" \
]
