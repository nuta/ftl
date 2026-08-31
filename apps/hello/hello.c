#include <arpa/inet.h>
#include <errno.h>
#include <stdio.h>
#include <string.h>
#include <sys/socket.h>
#include <unistd.h>

int main(void) {
    int listen_sock = socket(AF_INET, SOCK_STREAM, 0);
    if (listen_sock < 0) {
        fprintf(stderr, "socket failed: %s\n", strerror(errno));
        return 1;
    }

    struct sockaddr_in addr = {
        .sin_family = AF_INET,
        .sin_port = htons(80),
        .sin_addr.s_addr = htonl(INADDR_ANY),
    };

    if (bind(listen_sock, (struct sockaddr *) &addr, sizeof(addr)) < 0) {
        fprintf(stderr, "bind failed: %s\n", strerror(errno));
        return 1;
    }

    int backlog = 16;
    if (listen(listen_sock, backlog) < 0) {
        fprintf(stderr, "listen failed: %s\n", strerror(errno));
        return 1;
    }

    printf("HTTP server listening on port 80\n");
    for (;;) {
        int sock = accept(listen_sock, NULL, NULL);
        if (sock < 0) {
            fprintf(stderr, "accept failed: %s\n", strerror(errno));
            continue;
        }

        char buf[1024];
        ssize_t n = read(sock, buf, sizeof(buf));
        if (n > 0) {
            static const char response[] =
                "HTTP/1.1 200 OK\r\n"
                "Content-Length: 15\r\n"
                "Content-Type: text/plain\r\n"
                "Connection: close\r\n"
                "\r\n"
                "Hello from FTL\n";

            if (write(sock, response, sizeof(response) - 1) < 0) {
                fprintf(stderr, "sock write failed: %s\n", strerror(errno));
            }
        }

        close(sock);
    }
}
