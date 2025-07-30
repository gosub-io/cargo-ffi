#include <stdio.h>
#include <stdlib.h>
#include "gosub.h"

int main() {
    printf("Hello, this is the C example for Gosub!\n");

    GosubEngineHandle engine = gosub_engine_new();
    gosub_load_url(engine, "https://example.com");

    for (int i=0; i < 10; i++) {
        if (gosub_tick(engine)) {
            uint8_t buffer[4];
            gosub_render(engine, buffer, 4);

            printf("Pixel: %02x %02x %02x %02x\n", buffer[0], buffer[1], buffer[2], buffer[3]);
        }
    }

    gosub_engine_free(engine);
    return 0;
}