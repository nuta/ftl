FROM golang:latest AS builder

RUN mkdir /build
COPY <<-"EOF" /build/main.go
package main

import (
	"context"
	"fmt"
	"log"
	"net/http"
	"net/http/httputil"
	"net/url"
	"os"
	"os/exec"
	"os/signal"
	"time"
	"syscall"
)

func main() {
    fmt.Println("[supervisor] starting...")

	cmd := exec.Command("qemu-system-riscv64",
        "-kernel", "/app/ftl.elf",
		"-nographic", "-serial", "mon:stdio", "--no-reboot",
		"-machine", "virt", "-m", "256", "-bios", "default",
		"-global", "virtio-mmio.force-legacy=false",
		"-device", "virtio-net-device,netdev=net0,bus=virtio-mmio-bus.0",
		"-netdev", "user,id=net0,hostfwd=tcp:127.0.0.1:1234-:80")

	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr

	if err := cmd.Start(); err != nil {
		log.Fatalf("Failed to start QEMU: %v", err)
	}

	target, _ := url.Parse("http://127.0.0.1:1234")
	proxy := httputil.NewSingleHostReverseProxy(target)

    fmt.Println("[supervisor] QEMU started")
	http.HandleFunc("/", func(w http.ResponseWriter, r *http.Request) {
        fmt.Printf("[supervisor] %s %s %s\n", r.RemoteAddr, r.Method, r.URL)
		proxy.ServeHTTP(w, r)
	})

	server := &http.Server{
		Addr: "0.0.0.0:8080",
	}

	stop := make(chan os.Signal, 1)
	signal.Notify(stop, os.Interrupt, syscall.SIGTERM)

	go func() {
		if err := server.ListenAndServe(); err != nil && err != http.ErrServerClosed {
			log.Fatalf("HTTP server error: %v", err)
		}
	}()

	<-stop

	fmt.Println("[supervisor] shutting down gracefully...")
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	if err := server.Shutdown(ctx); err != nil {
		log.Printf("HTTP server shutdown error: %v", err)
	}

	if err := cmd.Process.Signal(syscall.SIGTERM); err != nil {
		log.Printf("Failed to terminate QEMU: %v", err)
		cmd.Process.Kill()
	}

	cmd.Wait()
	fmt.Println("Shutdown complete")
}
EOF

RUN go build -o /build/supervisor /build/main.go

FROM ubuntu:24.04
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y qemu-system-riscv64

RUN mkdir /app
WORKDIR /app
COPY --from=builder /build/supervisor /app/supervisor
COPY ftl.elf /app/ftl.elf
EXPOSE 8080
CMD ["/app/supervisor"]
