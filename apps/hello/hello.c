#include <stdio.h>
#include <string.h>
#include <unistd.h>
#include <errno.h>

int main(void)
{
    pid_t pid = fork();
    if (pid < 0)
    {
        printf("failed to fork: %s\n", strerror(errno));
        return 1;
    }

    if (pid == 0)
        printf("Hello from child\n");
    else
        printf("Hello from parent child=%d\n", pid);

    return 0;
}
