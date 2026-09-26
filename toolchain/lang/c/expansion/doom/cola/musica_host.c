/* musica_host.c -- OIR LA MUSICA DE DOOM EN WINDOWS, ANTES DEL METAL.
 *
 * Compila `bmo_mus.c` + `bmo_opl.c` (los MISMOS ficheros que entran en
 * doom.bex) con cl, lee un lump MUS y el GENMIDI de doom1.wad y escribe un
 * .wav de 16 bits mono. Si suena mal aqui, sonara mal en el Ryzen: esto es el
 * banco de pruebas del sintetizador, y no necesita flashear nada.
 *
 *   musica_host.exe <doom1.wad> <D_E1M1> <salida.wav> [hz]
 *
 * Tambien dice lo que el metal no puede decir a ojo: cuanto tarda en
 * generarse cada segundo de musica, cuantas muestras se sujetaron y cuantas
 * notas tuvieron que robar voz. */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#include "../doom/doomgeneric/doomgeneric/bmo_mus.c"

static unsigned char *leer_todo(const char *ruta, long *largo) {
    FILE *f = fopen(ruta, "rb");
    unsigned char *b;
    if (!f) {
        return 0;
    }
    fseek(f, 0, SEEK_END);
    *largo = ftell(f);
    fseek(f, 0, SEEK_SET);
    b = (unsigned char *)malloc(*largo);
    fread(b, 1, *largo, f);
    fclose(f);
    return b;
}

static int le32(const unsigned char *p) {
    return p[0] | (p[1] << 8) | (p[2] << 16) | (p[3] << 24);
}

static const unsigned char *lump(const unsigned char *wad, const char *nombre, int *largo) {
    int n = le32(wad + 4);
    int dir = le32(wad + 8);
    int i;
    for (i = 0; i < n; i++) {
        const unsigned char *e = wad + dir + 16 * i;
        char nom[9];
        memcpy(nom, e + 8, 8);
        nom[8] = 0;
        if (_stricmp(nom, nombre) == 0) {
            *largo = le32(e + 4);
            return wad + le32(e);
        }
    }
    return 0;
}

static void pon16(FILE *f, int v) {
    fputc(v & 0xFF, f);
    fputc((v >> 8) & 0xFF, f);
}

static void pon32(FILE *f, unsigned int v) {
    pon16(f, v & 0xFFFF);
    pon16(f, (v >> 16) & 0xFFFF);
}

int main(int argc, char **argv) {
    long wlargo;
    unsigned char *wad;
    const unsigned char *gm;
    const unsigned char *mus;
    int glargo;
    int mlargo;
    int hz = argc > 4 ? atoi(argv[4]) : 24000;
    unsigned long long total;
    int *pcm;
    int hechas;
    FILE *f;
    clock_t t0;
    double seg;
    int i;
    int pico = 0;

    if (argc < 4) {
        fprintf(stderr, "uso: musica_host <wad> <lump> <salida.wav> [hz]\n");
        return 2;
    }
    wad = leer_todo(argv[1], &wlargo);
    if (!wad) {
        fprintf(stderr, "no se lee %s\n", argv[1]);
        return 1;
    }
    gm = lump(wad, "GENMIDI", &glargo);
    mus = lump(wad, argv[2], &mlargo);
    /* ** UNA NOTA SOLA: `NOTA <instr> <tecla>` escribe una partitura de un
     * segundo con ese instrumento y esa tecla, para medir su frecuencia. */
    if (_stricmp(argv[2], "NOTA") == 0 && argc >= 6) {
        static unsigned char una[64];
        int n = 0;
        int instr = atoi(argv[4]);
        int tecla = atoi(argv[5]);
        int canal = instr >= 128 ? 15 : 0;
        memcpy(una, "MUS\x1a", 4);
        n = 16;
        una[6] = 16;
        if (canal == 0) {
            una[n++] = 0x40;            /* controlador, canal 0 */
            una[n++] = 0;               /* 0 = instrumento */
            una[n++] = (unsigned char)instr;
        } else {
            tecla = instr - 128 + 35;   /* la percusion ES la tecla */
        }
        una[n++] = (unsigned char)(0x90 | canal); /* nota, ultimo del grupo */
        una[n++] = (unsigned char)(0x80 | tecla);
        una[n++] = 127;
        una[n++] = 0x81;                /* 140 tics: un segundo */
        una[n++] = 0x0C;
        una[n++] = (unsigned char)(0x80 | canal); /* soltar, ultimo */
        una[n++] = (unsigned char)tecla;
        una[n++] = 0x14;                /* 20 tics de cola */
        una[n++] = 0x60;                /* fin */
        una[4] = (unsigned char)(n - 16);
        mus = una;
        mlargo = n;
        hz = 24000;
    }
    if (!gm || !mus) {
        fprintf(stderr, "falta GENMIDI o %s\n", argv[2]);
        return 1;
    }
    if (!bmo_mus_instrumentos(gm, glargo)) {
        fprintf(stderr, "el GENMIDI no vale\n");
        return 1;
    }
    total = bmo_mus_empezar(mus, mlargo, hz);
    if (total == 0) {
        fprintf(stderr, "%s no es un MUS\n", argv[2]);
        return 1;
    }
    pcm = (int *)malloc(sizeof(int) * (size_t)total);
    t0 = clock();
    hechas = 0;
    while ((unsigned long long)hechas < total) {
        int k = bmo_mus_tocar(pcm + hechas, 3600);
        if (k == 0) {
            break;
        }
        hechas += k;
    }
    seg = (double)(clock() - t0) / CLOCKS_PER_SEC;
    for (i = 0; i < hechas; i++) {
        int a = pcm[i] < 0 ? -pcm[i] : pcm[i];
        if (a > pico) {
            pico = a;
        }
    }

    f = fopen(argv[3], "wb");
    fwrite("RIFF", 1, 4, f);
    pon32(f, 36 + (unsigned int)hechas * 2);
    fwrite("WAVEfmt ", 1, 8, f);
    pon32(f, 16);
    pon16(f, 1);
    pon16(f, 1);
    pon32(f, (unsigned int)hz);
    pon32(f, (unsigned int)hz * 2);
    pon16(f, 2);
    pon16(f, 16);
    fwrite("data", 1, 4, f);
    pon32(f, (unsigned int)hechas * 2);
    for (i = 0; i < hechas; i++) {
        pon16(f, pcm[i]);
    }
    fclose(f);

    printf("%s: %llu muestras previstas, %d hechas (%.1f s a %d Hz)\n", argv[2], total, hechas,
           (double)hechas / hz, hz);
    printf("  generado en %.3f s (x%.0f tiempo real), pico %d de 32767, sujetadas %llu, robadas %u\n",
           seg, seg > 0 ? ((double)hechas / hz) / seg : 0.0, pico, bmo_opl_recortes, bmo_mus_robadas);
    return 0;
}
