/* Solo para MEDIR: el nucleo de DOOM mas un backend vacio. No dibuja. */
#include "bmo_unity.c"

pixel_t *DG_ScreenBuffer = 0;
void DG_Init() { }
void DG_DrawFrame() { }
void DG_SleepMs(uint32_t ms) { }
uint32_t DG_GetTicksMs() { return 0; }
int DG_GetKey(int *pressed, unsigned char *key) { return 0; }
void DG_SetWindowTitle(const char *title) { }

int main() {
    doomgeneric_Create(0, 0);
    return 0;
}
