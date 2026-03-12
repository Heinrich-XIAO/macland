#include <stdio.h>
#include <stdlib.h>
#include <string.h>

int main(int argc, char **argv) {
    if (argc > 1 && strcmp(argv[1], "--self-test") == 0) {
        return 0;
    }

    const char *mode = getenv("MACLAND_MODE");
    if (mode == NULL) {
        mode = "unset";
    }

    printf("meson-compositor:%s\n", mode);
    return 0;
}
