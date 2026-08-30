#include <arpa/inet.h>
#include <errno.h>
#include <stdio.h>
#include <string.h>
#include <sys/socket.h>
#include <unistd.h>

int main(void)
{
    int listener = socket(AF_INET, SOCK_STREAM, 0);
    if (listener < 0) {
        printf("socket failed: %s\n", strerror(errno));
        return 1;
    }

    struct sockaddr_in address = {
        .sin_family = AF_INET,
        .sin_port = htons(80),
        .sin_addr.s_addr = htonl(INADDR_ANY),
    };
    if (bind(listener, (struct sockaddr *)&address, sizeof(address)) < 0) {
        printf("bind failed: %s\n", strerror(errno));
        return 1;
    }
    if (listen(listener, 8) < 0) {
        printf("listen failed: %s\n", strerror(errno));
        return 1;
    }

    printf("HTTP server listening on port 80\n");
    for (;;) {
        int connection = accept(listener, NULL, NULL);
        if (connection < 0) {
            printf("accept failed: %s\n", strerror(errno));
            continue;
        }

        char request[1024];
        ssize_t received = read(connection, request, sizeof(request));
        if (received > 0) {
            static const char response[] =
                "HTTP/1.1 200 OK\r\n"
                "Content-Length: 15\r\n"
                "Content-Type: text/plain\r\n"
                "Connection: close\r\n"
                "\r\n"
                "Hello from FTL\n";
            write(connection, response, sizeof(response) - 1);
        }
        close(connection);
    }
}
