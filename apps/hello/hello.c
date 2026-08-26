#include <stdio.h>
#include <string.h>
#include <unistd.h>
#include <errno.h>

int main(int argc, char **argv)
{
    if (argc > 1)
    {
        for (int i = 0; i < argc; i++)
        {
            printf("argv[%d] = %s\n", i, argv[i]);
        }
        return 0;
    }

    pid_t pid = fork();
    if (pid < 0)
    {
        printf("failed to fork: %s\n", strerror(errno));
        return 1;
    }

    if (pid == 0)
    {
        printf("Hello from child\n");
        char *const argv[] = {"hello.elf", "exec", NULL};
        execve("hello.elf", argv, NULL);
        printf("failed to exec: %s\n", strerror(errno));
    }
    else
        printf("Hello from parent child=%d\n", pid);

    return 0;
}
