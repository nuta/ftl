/* A simple HTTP server */
#include <arpa/inet.h>
#include <errno.h>
#include <fcntl.h>
#include <poll.h>
#include <stdio.h>
#include <string.h>
#include <sys/socket.h>
#include <unistd.h>

#define MAX_CLIENTS 16
#define MAX_REQUEST_SIZE 4096

#define STRINGIFY_(value) #value
#define STRINGIFY(value) STRINGIFY_(value)
#define RESPONSE_HEADER(status, content_length) \
    "HTTP/1.0 " status "\r\n" \
    "X-Powered-By: FTL\r\n" \
    "Content-Length: " STRINGIFY(content_length) "\r\n" \
    "Content-Type: text/html\r\n" \
    "Connection: close\r\n" \
    "\r\n"

static const unsigned char index_html_body[] = {
#embed "index.html"
};

static const unsigned char not_found_html_body[] = {
#embed "404.html"
};

static const char index_header[] = RESPONSE_HEADER("200 OK", INDEX_HTML_LENGTH);
static const char not_found_header[] = RESPONSE_HEADER("404 Not Found", NOT_FOUND_HTML_LENGTH);

struct response {
    const char *header;
    size_t header_len;
    const unsigned char *body;
    size_t body_len;
};

static const struct response index_html = {
    .header = index_header,
    .header_len = sizeof(index_header) - 1,
    .body = index_html_body,
    .body_len = sizeof(index_html_body),
};

static const struct response not_found_html = {
    .header = not_found_header,
    .header_len = sizeof(not_found_header) - 1,
    .body = not_found_html_body,
    .body_len = sizeof(not_found_html_body),
};

struct client {
    int sock;
    char request[MAX_REQUEST_SIZE + 1];
    size_t read_len;
    size_t written_len;
    const struct response *response;
};

static int set_nonblocking(int fd) {
    int flags = fcntl(fd, F_GETFL, 0);
    if (flags < 0) {
        fprintf(stderr, "fcntl(F_GETFL) failed: %s\n", strerror(errno));
        return -1;
    }

    if (fcntl(fd, F_SETFL, flags | O_NONBLOCK) < 0) {
        fprintf(stderr, "fcntl(F_SETFL) failed: %s\n", strerror(errno));
        return -1;
    }

    return 0;
}

static int request_complete(const struct client *client) {
    return strstr(client->request, "\r\n\r\n") != NULL;
}

static void close_client(struct client *client) {
    close(client->sock);
    *client = (struct client) {.sock = -1};
}

static void write_response(struct client *c) {
    const struct response *r = c->response;
    const char *buf;
    size_t len;
    if (c->written_len < r->header_len) {
        buf = &r->header[c->written_len];
        len = r->header_len - c->written_len;
    } else {
        size_t body_offset = c->written_len - r->header_len;
        buf = (const char *) &r->body[body_offset];
        len = r->body_len - body_offset;
    }

    ssize_t n = write(c->sock, buf, len);
    if (n < 0) {
        fprintf(stderr, "write failed: n=%zd, errno=%d\n", n, errno);
        return;
    }

    if (n <= 0) {
        close_client(c);
        return;
    }

    c->written_len += n;
    if (c->written_len == r->header_len + r->body_len) {
        close_client(c);
    }
}


static void read_request(struct client *c) {
    char *buf = &c->request[c->read_len];
    size_t len = MAX_REQUEST_SIZE - c->read_len;
    ssize_t n = read(c->sock, buf, len);
    if (n < 0) {
        fprintf(stderr, "read failed: n=%zd, errno=%d\n", n, errno);
        return;
    }

    if (n <= 0) {
        close_client(c);
        return;
    }

    c->read_len += n;
    c->request[c->read_len] = '\0';
    if (request_complete(c)) {
        // A super simple request parser.
        if (strstr(c->request, "GET / ") != NULL ||
            strstr(c->request, "GET /index.html ") != NULL) {
            c->response = &index_html;
        } else {
            c->response = &not_found_html;
        }
        return;
    }

    if (c->read_len == MAX_REQUEST_SIZE) {
        fprintf(stderr, "request too large\n");
        close_client(c);
    }
}

static void handle_client(struct client *client, short revents) {
    if (revents & POLLOUT) {
        if (client->response != NULL) {
            write_response(client);
            return;
        }
    }

    if (revents & POLLIN) {
        read_request(client);
    }
}

struct client clients[MAX_CLIENTS];
struct pollfd pfds[1 + MAX_CLIENTS];
int index2client[1 + MAX_CLIENTS];

static int build_pollfds(
    int listen_sock,
    const struct client clients[],
    struct pollfd pfds[],
    int index2client[]
) {
    pfds[0] = (struct pollfd) {.fd = listen_sock, .events = POLLIN};
    int nfds = 1;
    for (int i = 0; i < MAX_CLIENTS; i++) {
        if (clients[i].sock < 0) {
            continue;
        }

        pfds[nfds].fd = clients[i].sock;
        pfds[nfds].events = clients[i].response == NULL ? POLLIN : POLLOUT;
        index2client[nfds] = i;
        nfds++;
    }
    return nfds;
}

static int find_free_slot(const struct client clients[]) {
    for (int i = 0; i < MAX_CLIENTS; i++) {
        if (clients[i].sock < 0) {
            return i;
        }
    }
    return -1;
}

static void accept_clients(int listen_sock, struct client clients[]) {
    for (;;) {
        int sock = accept(listen_sock, NULL, NULL);
        if (sock < 0) {
            if (errno != EAGAIN && errno != EWOULDBLOCK) {
                fprintf(stderr, "accept failed: %s\n", strerror(errno));
            }
            return;
        }

        int slot = find_free_slot(clients);
        if (slot < 0 || set_nonblocking(sock) < 0) {
            close(sock);
            continue;
        }
        clients[slot] = (struct client) {.sock = sock};
    }
}

int main(void) {
    int listen_sock = socket(AF_INET, SOCK_STREAM, 0);
    if (listen_sock < 0) {
        fprintf(stderr, "socket failed: %s\n", strerror(errno));
        return -1;
    }

    struct sockaddr_in addr = {
        .sin_family = AF_INET,
        .sin_port = htons(80),
        .sin_addr.s_addr = htonl(INADDR_ANY),
    };
    if (bind(listen_sock, (struct sockaddr *) &addr, sizeof(addr)) < 0) {
        fprintf(stderr, "bind failed: %s\n", strerror(errno));
        close(listen_sock);
        return 1;
    }

    if (listen(listen_sock, MAX_CLIENTS) < 0) {
        fprintf(stderr, "listen failed: %s\n", strerror(errno));
        close(listen_sock);
        return 1;
    }

    if (set_nonblocking(listen_sock) < 0) {
        fprintf(stderr, "set_nonblocking failed: %s\n", strerror(errno));
        close(listen_sock);
        return 1;
    }

    printf("HTTP server listening on port 80\n");
    for (int i = 0; i < MAX_CLIENTS; i++) {
        clients[i].sock = -1;
    }

    for (;;) {
        int nfds = build_pollfds(listen_sock, clients, pfds, index2client);
        if (poll(pfds, nfds, -1) < 0) {
            fprintf(stderr, "poll failed: %s\n", strerror(errno));
            continue;
        }
        
        // listen socket
        if (pfds[0].revents & POLLIN) {
            accept_clients(listen_sock, clients);
        }

        // client sockets
        for (int i = 1; i < nfds; i++) {
            handle_client(&clients[index2client[i]], pfds[i].revents);
        }
    }
}
