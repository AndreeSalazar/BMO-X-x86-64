/* bmo_musica.c -- LA MUSICA DE DOOM EN BMO-X: la cancion ENTERA en el banco,
 * y el orquestador la toca en bucle (2026-09-22).
 *
 * Lo incluye `bmo_sonido.c`, detras de los efectos: usa su capability, su
 * banco y su `bmo_snd_va`. El sintetizador y la partitura viven en
 * `bmo_opl.c` y `bmo_mus.c`, que no saben de DOOM (y por eso se prueban en
 * Windows con `doom-port/musica_host.c`).
 *
 * == El reparto, y por que no es un flujo ==
 *
 * Una cancion podria ir por el ANILLO del tubo, generada en vivo. Seria volver
 * al problema que las voces acaban de quitar: si DOOM se atasca --el
 * derretido, cargar un mapa-- la musica se corta. Asi que va por el mismo
 * camino que los efectos, llevado al extremo:
 *
 *     DOOM pide la cancion          se RENDERIZA en el banco (24 kHz, S16, mono)
 *     los primeros 6 s, al momento  y se toca como UNA voz en BUCLE
 *     el resto, en `Poll`: solo lo que falte para ir 8 s por delante
 *
 * El orquestador la toca a su ritmo. DOOM solo tiene que ir por delante.
 * ** Y el "le sobra" que decia aqui era de Windows (cl, ~500 veces el tiempo
 * real): en el Ryzen, con BMO C, 150 ms de musica costaron ~14 ms por
 * fotograma y DOOM cayo a 31 fps. De ahi el presupuesto de abajo, y la linea
 * que el metal escribe al acabar cada cancion con su coste real.
 *
 * == El banco ==
 *
 *     0 .. 2 MiB      los efectos (`bmo_sonido.c`)
 *     2 .. 16 MiB     la cancion que suena: 305 s a 24 kHz. La mas larga de
 *                     doom1.wad (D_E1M3) son 272 s
 *
 * == Lo que no hace, y se dice ==
 *
 *   - Una cancion de mas de 305 s se CORTA ahi (y lo dice).
 *   - Al volver al principio se oye el corte de la ultima nota: la cola que
 *     seguiria sonando no esta en el banco.
 *   - Pausar baja la voz a cero: la cancion sigue corriendo callada, no se
 *     para en su sitio (las voces no tienen pausa, y no se agrega un verbo
 *     para esto). */

#include "bmo_mus.c"

/* La frecuencia de la musica: exactamente la mitad de los 48 kHz del tubo, asi
 * que el orquestador la sube con una recta entre muestras sin fraccion. */
#define BMO_MUS_HZ 24000
/* El canal de voz de la musica. DOOM usa del 0 al 7 para los efectos. */
#define BMO_MUS_VOZ 15
#define BMO_MUS_BASE BMO_SND_EFECTOS_BYTES
#define BMO_MUS_MAX ((BMO_SND_BANCO - BMO_MUS_BASE) / 2)
/* Lo que se renderiza de golpe al empezar: 6 s. El derretido son ~1,5. */
#define BMO_MUS_ADELANTO (6 * BMO_MUS_HZ)
/* El trozo del bufer de trabajo: 150 ms de musica. */
#define BMO_MUS_TROZO 3600
/* ** LO QUE SE RENDERIZA POR VUELTA NO ES UNA CANTIDAD: ES UN TIEMPO
 * (2026-09-22). La primera version hacia 150 ms de musica por vuelta, y el
 * metal lo conto en el `[perf]`: al cambiar de mapa DOOM bajo de 57 a 31 fps
 * durante casi un minuto (el fotograma paso de 17,4 a 31 ms). La musica se
 * comia 14 ms por fotograma. Ahora se hacen rebanadas de 20 ms de musica
 * hasta gastar el presupuesto, y solo se apura si la voz esta a menos de 2 s
 * de alcanzar lo hecho. */
#define BMO_MUS_REBANADA 480
#define BMO_MUS_PRESUPUESTO_MS 2
#define BMO_MUS_APURO_MS 8
/* ** Y NO SE RENDERIZA LO MAS RAPIDO POSIBLE: SOLO LO NECESARIO (16:27 del
 * 22-09). Con el presupuesto de 2 ms el metal dijo 44-45 fps durante TODA la
 * sesion (fotograma 22 ms en vez de 17,4): el render iba a ~2,5 veces el
 * tiempo real y cobraba el presupuesto entero en cada fotograma mientras la
 * cancion no estuviera hecha. Ahora solo se renderiza hasta ir MARGEN por
 * delante de lo que suena: en regimen es lo que suena en un fotograma, ~500
 * muestras. */
#define BMO_MUS_MARGEN (8 * BMO_MUS_HZ)

static int bmo_musica_ok;
static const unsigned char *bmo_musica_datos;
static int bmo_musica_largo;
static int bmo_musica_suena;
static int bmo_musica_bucle;
static int bmo_musica_pausa;
/* El volumen de DOOM (0..127) ya en lados de voz (0..256). */
static int bmo_musica_lado = 128;
static unsigned long long bmo_musica_total;
static unsigned long long bmo_musica_hechas;
/* Cuando empezo a sonar (ms), para saber si el renderizado va por detras. */
static int bmo_musica_desde;
static int bmo_musica_ms;
static int bmo_musica_atrasos;
static int bmo_musica_trozo[BMO_MUS_TROZO];

/* Lo renderizado, al banco, en 16 bits little endian. */
static void bmo_musica_renderizar(int cuantas) {
    int n;
    int i;
    unsigned char *d;

    while (cuantas > 0 && bmo_musica_hechas < bmo_musica_total) {
        n = cuantas;
        if (n > BMO_MUS_TROZO) {
            n = BMO_MUS_TROZO;
        }
        if ((unsigned long long)n > bmo_musica_total - bmo_musica_hechas) {
            n = (int)(bmo_musica_total - bmo_musica_hechas);
        }
        n = bmo_mus_tocar(bmo_musica_trozo, n);
        if (n <= 0) {
            /* La partitura se acabo antes de lo contado: lo que queda del
             * banco ya esta a cero. */
            bmo_musica_hechas = bmo_musica_total;
            return;
        }
        d = bmo_snd_banco + BMO_MUS_BASE + bmo_musica_hechas * 2;
        for (i = 0; i < n; i++) {
            int v = bmo_musica_trozo[i];
            d[2 * i] = (unsigned char)(v & 0xFF);
            d[2 * i + 1] = (unsigned char)((v >> 8) & 0xFF);
        }
        bmo_musica_hechas = bmo_musica_hechas + (unsigned long long)n;
        cuantas = cuantas - n;
    }
}

/* **Que suene**: la voz de la musica, si hay tubo. Lo llaman `PlaySong` y el
 * tubo cuando por fin entra (o vuelve). */
static void bmo_musica_arrancar(void) {
    if (!bmo_snd_va || !bmo_musica_suena || bmo_musica_total == 0) {
        return;
    }
    if (bmo_musica_bucle) {
        (void)bmo_voz_tocar_bucle(bmo_snd_cap, BMO_MUS_VOZ, BMO_MUS_BASE, bmo_musica_total,
                                  BMO_VOZ_S16, BMO_MUS_HZ, 0, 0, 0);
    } else {
        (void)bmo_voz_tocar(bmo_snd_cap, BMO_MUS_VOZ, BMO_MUS_BASE, bmo_musica_total,
                            BMO_VOZ_S16, BMO_MUS_HZ, 0, 0, 0);
    }
    /* Arranca CALLADA y se sube aqui: asi el volumen tiene un solo sitio
     * donde se decide, que es este. */
    if (!bmo_musica_pausa) {
        (void)bmo_voz_ajustar(bmo_snd_cap, BMO_MUS_VOZ, bmo_musica_lado, bmo_musica_lado);
    }
    bmo_musica_desde = I_GetTimeMS();
}

static boolean I_BMO_InitMusic(void) {
    int num;
    int largo;
    const unsigned char *gm;

    num = W_CheckNumForName("GENMIDI");
    if (num < 0) {
        printf("[bmo] musica: este WAD no trae GENMIDI; DOOM sin musica\n");
        return false;
    }
    largo = W_LumpLength(num);
    gm = (const unsigned char *)W_CacheLumpNum(num, PU_STATIC);
    if (!bmo_mus_instrumentos(gm, largo)) {
        printf("[bmo] musica: el GENMIDI no es un banco OPL; DOOM sin musica\n");
        return false;
    }
    if (!bmo_snd_banco_pedir()) {
        return false;
    }
    bmo_musica_ok = 1;
    printf("[bmo] musica: FM de 18 voces con el GENMIDI del WAD, %d Hz, en bucle por el orquestador\n",
           BMO_MUS_HZ);
    return true;
}

static void I_BMO_StopSong(void) {
    if (bmo_musica_suena && bmo_snd_va) {
        (void)bmo_voz_callar(bmo_snd_cap, BMO_MUS_VOZ);
    }
    bmo_musica_suena = 0;
}

static void I_BMO_ShutdownMusic(void) {
    I_BMO_StopSong();
}

static void I_BMO_SetMusicVolume(int v) {
    if (v < 0) {
        v = 0;
    }
    if (v > 127) {
        v = 127;
    }
    bmo_musica_lado = v * 256 / 127;
    if (bmo_musica_suena && !bmo_musica_pausa && bmo_snd_va) {
        (void)bmo_voz_ajustar(bmo_snd_cap, BMO_MUS_VOZ, bmo_musica_lado, bmo_musica_lado);
    }
}

static void I_BMO_PauseSong(void) {
    bmo_musica_pausa = 1;
    if (bmo_musica_suena && bmo_snd_va) {
        (void)bmo_voz_ajustar(bmo_snd_cap, BMO_MUS_VOZ, 0, 0);
    }
}

static void I_BMO_ResumeSong(void) {
    bmo_musica_pausa = 0;
    if (bmo_musica_suena && bmo_snd_va) {
        (void)bmo_voz_ajustar(bmo_snd_cap, BMO_MUS_VOZ, bmo_musica_lado, bmo_musica_lado);
    }
}

/* DOOM entrega el lump MUS tal cual. El asa es el propio dato. */
static void *I_BMO_RegisterSong(void *datos, int largo) {
    const unsigned char *m = (const unsigned char *)datos;

    if (!bmo_musica_ok || m == 0 || largo < 16 || m[0] != 'M' || m[1] != 'U' || m[2] != 'S'
        || m[3] != 0x1A) {
        return 0;
    }
    bmo_musica_datos = m;
    bmo_musica_largo = largo;
    return datos;
}

/* ** DOOM suelta el lump DESPUES de esto (`W_ReleaseLumpNum`), asi que desde
 * aqui ya no se puede leer: se deja de renderizar. */
static void I_BMO_UnRegisterSong(void *asa) {
    if (asa != 0 && asa == (void *)bmo_musica_datos) {
        I_BMO_StopSong();
        bmo_musica_datos = 0;
        bmo_musica_total = 0;
        bmo_musica_hechas = 0;
    }
}

static void I_BMO_PlaySong(void *asa, boolean bucle) {
    int t0;
    unsigned long long total;

    if (asa == 0 || asa != (void *)bmo_musica_datos || !bmo_musica_ok) {
        return;
    }
    I_BMO_StopSong();
    total = bmo_mus_empezar(bmo_musica_datos, bmo_musica_largo, BMO_MUS_HZ);
    if (total == 0) {
        printf("[bmo] musica: la partitura no se deja leer\n");
        return;
    }
    if (total > (unsigned long long)BMO_MUS_MAX) {
        printf("[bmo] musica: la cancion dura %d s y en el banco caben %d: se corta ahi\n",
               (int)(total / BMO_MUS_HZ), (int)(BMO_MUS_MAX / BMO_MUS_HZ));
        total = (unsigned long long)BMO_MUS_MAX;
    }
    /* A cero lo que va a sonar: si el render se quedara atras, se oiria
     * silencio y no la cancion anterior. */
    memset(bmo_snd_banco + BMO_MUS_BASE, 0, (size_t)(total * 2));
    bmo_musica_total = total;
    bmo_musica_hechas = 0;
    bmo_musica_atrasos = 0;
    t0 = I_GetTimeMS();
    bmo_musica_renderizar(BMO_MUS_ADELANTO);
    bmo_musica_ms = I_GetTimeMS() - t0;
    bmo_musica_suena = 1;
    bmo_musica_bucle = bucle ? 1 : 0;
    bmo_musica_arrancar();
}

/* **La vuelta**: rebanadas de la cancion hasta gastar el presupuesto, siempre
 * por delante de la voz. Y si la voz la alcanza, se DICE: se habria oido
 * silencio. */
static void I_BMO_PollMusic(void) {
    int t0;
    int limite;
    unsigned long long va_por;

    if (!bmo_musica_suena || bmo_musica_hechas >= bmo_musica_total) {
        return;
    }
    t0 = I_GetTimeMS();
    limite = BMO_MUS_PRESUPUESTO_MS;
    /* Por donde va la voz. Sin tubo la voz no ha empezado: cero, y el margen
     * se llena una vez y se para. */
    va_por = 0;
    if (bmo_snd_va) {
        va_por = (unsigned long long)(t0 - bmo_musica_desde) * BMO_MUS_HZ / 1000;
    }
    if (bmo_musica_hechas >= va_por + BMO_MUS_MARGEN) {
        return;
    }
    if (bmo_musica_hechas < va_por + 2 * BMO_MUS_HZ) {
        limite = BMO_MUS_APURO_MS;
    }
    /* Rebanadas de 20 ms de musica hasta tener el MARGEN delante, o hasta
     * gastar el presupuesto (el reloj va a milisegundos enteros: al menos una
     * rebanada pasa siempre). */
    do {
        bmo_musica_renderizar(BMO_MUS_REBANADA);
    } while (bmo_musica_hechas < bmo_musica_total && bmo_musica_hechas < va_por + BMO_MUS_MARGEN
             && (I_GetTimeMS() - t0) < limite);
    bmo_musica_ms = bmo_musica_ms + (I_GetTimeMS() - t0);
    if (bmo_snd_va && !bmo_musica_atrasos) {
        va_por = (unsigned long long)(I_GetTimeMS() - bmo_musica_desde) * BMO_MUS_HZ / 1000;
        if (va_por > bmo_musica_hechas) {
            bmo_musica_atrasos = 1;
            printf("[bmo] musica: la voz ALCANZO al render (%d ms de musica sin hacer): sono silencio\n",
                   (int)((va_por - bmo_musica_hechas) * 1000 / BMO_MUS_HZ));
        }
    }
    if (bmo_musica_hechas >= bmo_musica_total) {
        /* El numero que juzga el sintetizador EN EL METAL: cuanto cuesta un
         * segundo de musica. En Windows (cl) son ~1 ms; lo que diga aqui es lo
         * que cuesta con BMO C en el Ryzen. */
        printf("[bmo] musica: %d s renderizados en %d ms = %d us por segundo de musica (sujetadas %d, notas sin voz %d)\n",
               (int)(bmo_musica_total / BMO_MUS_HZ), bmo_musica_ms,
               (int)((unsigned long long)bmo_musica_ms * 1000 * BMO_MUS_HZ / (bmo_musica_total + 1)),
               (int)bmo_opl_recortes, (int)bmo_mus_robadas);
    }
}

/* Una cancion SIN bucle se acaba sola en el orquestador: se le pregunta a la
 * voz, no a lo que se pidio. */
static boolean I_BMO_MusicIsPlaying(void) {
    if (!bmo_musica_suena) {
        return false;
    }
    if (!bmo_musica_bucle && bmo_snd_va) {
        return bmo_voz_suena(bmo_snd_cap, BMO_MUS_VOZ) ? true : false;
    }
    return true;
}

static snddevice_t bmo_aparatos_musica[] = {
    SNDDEVICE_SB,
    SNDDEVICE_ADLIB,
    SNDDEVICE_GENMIDI,
};

music_module_t DG_music_module = {
    bmo_aparatos_musica,
    sizeof(bmo_aparatos_musica) / sizeof(*bmo_aparatos_musica),
    I_BMO_InitMusic,
    I_BMO_ShutdownMusic,
    I_BMO_SetMusicVolume,
    I_BMO_PauseSong,
    I_BMO_ResumeSong,
    I_BMO_RegisterSong,
    I_BMO_UnRegisterSong,
    I_BMO_PlaySong,
    I_BMO_StopSong,
    I_BMO_MusicIsPlaying,
    I_BMO_PollMusic,
};
