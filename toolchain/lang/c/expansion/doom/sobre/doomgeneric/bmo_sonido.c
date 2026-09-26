/* bmo_sonido.c -- EL SONIDO DE DOOM EN BMO-X: DOOM DECLARA, EL ORQUESTADOR TOCA.
 *
 * El `sound_module_t` de DOOM sobre las VOCES DEL ORQUESTADOR (2026-09-22).
 *
 * == Por que este fichero ya no mezcla ==
 *
 * Hasta hoy aqui habia un mezclador entero: remuestrear de 11.025 a 48.000,
 * paneo, suma de ocho canales, recorte, un anillo de 256 KiB y la cuenta de
 * cuanto le quedaba al aparato. Y DOOM tenia que rellenar ese anillo SIEMPRE
 * con 100 ms de ventaja. Cuando DOOM se quedaba en un bucle suyo --la pantalla
 * que se derrite, cargar un mapa--, el anillo se vaciaba y el `save` lo conto:
 *
 *     en marcha 3.626    tirones 29    el mas largo 1.420 ms    tarde 0
 *
 * Tres arreglos seguidos (el latido, el limite, el reloj) curaron sintomas.
 * El propietario: *"mas elegante, por completo: por algo es BMO, Bare Metal
 * ORQUESTADOR"*. Lo que estaba mal era el REPARTO: la app marcaba el tiempo.
 *
 * Ahora DOOM hace lo que hace un juego con una tarjeta de voces (la Gravis
 * UltraSound y la AWE32, que DOOM ya nombra abajo en su lista de aparatos):
 *
 *     presta UN banco con los efectos        bmo_voz_banco
 *     dice "toca este, asi"                  bmo_voz_tocar
 *     mueve el lado cuando el monstruo anda  bmo_voz_ajustar
 *     y se olvida                            -- lo toca el kernel, cada 1 ms
 *
 * Si DOOM se atasca, lo que sonaba SIGUE sonando: ya no depende de el. Y del
 * disparo al ruido pasa de hasta 100 ms a lo que tarde una trama.
 *
 * == Este fichero NO esta en el repo de BMO, y no puede estarlo ==
 *
 * DOOM es GPL-2.0 y el arbol de BMO es Apache-2.0. Vive en `BMO-externo/`,
 * como `doomgeneric_bmo.c`. Lo que SI esta en el repo es todo lo que llama:
 * `<bmo/sonido.h>`, las voces (`dev/usb/voces.rs`) y su mezcla
 * (`bmo-amplificador::voces`).
 *
 * == Y LA MUSICA (2026-09-22) ==
 *
 * Tambien por las voces: `bmo_musica.c` renderiza la cancion ENTERA en el
 * banco con un sintetizador FM propio (`bmo_opl.c`) y los instrumentos del
 * GENMIDI del WAD, y el orquestador la toca en bucle en el canal 15.
 *
 * == Lo que NO hace, dicho aqui para no prometerlo ==
 *
 *   - Sonido posicional de verdad (HRTF): `sep` es un paneo de dos canales.
 *   - PISTAS para LA MESA: van a 0 hasta que exista su contrato (M3).
 */

#include <bmo/sonido.h>

/* ===================================================================
 *  1. Medidas, y de donde sale cada una
 * =================================================================== */

/* Los canales de DOOM. `snd_channels` vale 8; el orquestador tiene 16. */
#define BMO_SND_CANALES 8

/* La frecuencia de los efectos del WAD, para cuando la cabecera viene absurda. */
#define BMO_SND_HZ_WAD 11025

/* **El banco: 16 MiB, y UNO solo** (el kernel da un banco por programa).
 *
 *     0 .. 2 MiB    los efectos. Los de doom1.wad son 8 bits a 11.025 Hz y
 *                   no llegan a uno; el doble deja sitio a un WAD con mas.
 *                   Cada efecto entra la primera vez que suena y se queda
 *     2 .. 16 MiB   la cancion que suena (`bmo_musica.c`) */
#define BMO_SND_BANCO (16 * 1024 * 1024)
#define BMO_SND_EFECTOS_BYTES (2 * 1024 * 1024)

/* Cuantos efectos distintos caben en la tabla. doom1 trae unos sesenta. */
#define BMO_SND_EFECTOS 256

/* ===================================================================
 *  2. El estado
 * =================================================================== */

static unsigned long long bmo_snd_cap;
static int bmo_snd_va;               /* 1 = hay tubo y banco: se puede tocar */
/* Cuantas vueltas de `Update` faltan para volver a mirar si ya hay tubo. */
static int bmo_snd_espera;
/* Cuantas veces se ha mirado, para poder decirlo cuando por fin entre. */
static int bmo_snd_intentos;

/* El banco, y hasta donde esta lleno. */
static unsigned char *bmo_snd_banco;
static unsigned int bmo_snd_lleno;
/* Se dijo ya que el banco se lleno? Una vez basta. */
static int bmo_snd_avisado;

/* Lo que se sabe de cada efecto ya copiado. `driver_data` de su `sfxinfo_t`
 * guarda el indice + 1 -- es el campo que DOOM reserva para esto. */
typedef struct {
    unsigned int inicio;
    unsigned int muestras;
    unsigned int hz;
} BMO_EFECTO;
static BMO_EFECTO bmo_efecto[BMO_SND_EFECTOS];
static int bmo_efectos;

/* ===================================================================
 *  3. El lump: DMX, que es PCM con una cabecera de 24 bytes
 * ===================================================================
 *
 *     0..1   formato, siempre 3
 *     2..3   frecuencia (little endian), casi siempre 11025
 *     4..7   cuantas muestras, contando el relleno
 *     8..23  16 bytes de relleno (la primera muestra repetida)
 *     ...    las muestras, 8 bits SIN signo
 *     ultimos 16 bytes: mas relleno
 */
static int bmo_leer_dmx(const unsigned char *lump, int largo,
                        const unsigned char **muestras, unsigned int *n,
                        unsigned int *hz) {
    unsigned int declaradas;
    unsigned int frec;

    if (lump == 0 || largo < 32) {
        return 0;
    }
    if (lump[0] != 3 || lump[1] != 0) {
        /* No es DMX. No se adivina: se dice que no y ese efecto calla. */
        return 0;
    }
    frec = (unsigned int)lump[2] | ((unsigned int)lump[3] << 8);
    declaradas = (unsigned int)lump[4] | ((unsigned int)lump[5] << 8) |
                 ((unsigned int)lump[6] << 16) | ((unsigned int)lump[7] << 24);
    /* Lo que el lump DECLARA contra lo que de verdad hay: se toma el menor.
     * Un WAD tocado puede declarar mas de lo que trae. */
    if (declaradas > (unsigned int)(largo - 8)) {
        declaradas = (unsigned int)(largo - 8);
    }
    if (declaradas <= 32) {
        return 0;
    }
    *muestras = lump + 8 + 16;
    *n = declaradas - 32;
    *hz = (frec >= 4000 && frec <= 48000) ? frec : BMO_SND_HZ_WAD;
    return 1;
}

/* **Meter un efecto en el banco**, si no esta. Devuelve su indice o -1. */
static int bmo_snd_al_banco(sfxinfo_t *sfx) {
    const unsigned char *lump;
    const unsigned char *muestras;
    unsigned int n;
    unsigned int hz;
    unsigned int i;
    int largo;
    int k;

    if (sfx->driver_data != 0) {
        return (int)(unsigned long long)sfx->driver_data - 1;
    }
    if (bmo_efectos >= BMO_SND_EFECTOS) {
        return -1;
    }
    if (sfx->lumpnum < 0) {
        return -1;
    }
    largo = W_LumpLength(sfx->lumpnum);
    lump = (const unsigned char *)W_CacheLumpNum(sfx->lumpnum, PU_STATIC);
    if (!bmo_leer_dmx(lump, largo, &muestras, &n, &hz)) {
        return -1;
    }
    if (bmo_snd_lleno + n > BMO_SND_EFECTOS_BYTES) {
        /* Lleno. Se dice UNA vez y ese efecto no suena: vaciar el banco
         * callaria los que ya estan dentro. */
        if (!bmo_snd_avisado) {
            printf("[bmo] sonido: los %d KiB de efectos se llenaron; hay efectos que no sonaran\n",
                   BMO_SND_EFECTOS_BYTES / 1024);
            bmo_snd_avisado = 1;
        }
        return -1;
    }
    for (i = 0; i < n; i++) {
        bmo_snd_banco[bmo_snd_lleno + i] = muestras[i];
    }
    k = bmo_efectos;
    bmo_efecto[k].inicio = bmo_snd_lleno;
    bmo_efecto[k].muestras = n;
    bmo_efecto[k].hz = hz;
    bmo_efectos = bmo_efectos + 1;
    bmo_snd_lleno = bmo_snd_lleno + n;
    sfx->driver_data = (void *)(unsigned long long)(k + 1);
    return k;
}

/* ===================================================================
 *  4. Abrir: el tubo, y el banco
 * ===================================================================
 *
 * ** SE PUEDE LLAMAR MUCHAS VECES. El audifono de esta casa puede tardar medio
 * minuto en entrar --falla su primer intento de enumeracion y el puerto entra
 * en la escalera de descanso del bus: 5, 10, 20, 40 s--. Un tubo no es una
 * constante del arranque: es un aparato que se enchufa. Si no esta, se SUELTA
 * lo cogido (quedarse la capability dejaria mudo a cualquier otro) y se
 * vuelve a mirar. Devuelve 1 si quedo abierto. */
/* La musica vive mas abajo (`bmo_musica.c`), y el tubo la tiene que poder
 * arrancar en cuanto entra. */
static void bmo_musica_arrancar(void);

/* **El banco se pide UNA vez** y se queda aunque el tubo se vaya y vuelva: lo
 * que ya esta dentro no hay que volver a hacerlo. Lo piden los efectos y la
 * musica, y la musica lo necesita antes de que haya tubo: renderiza igual.
 *
 * ** Y no rompe `fread`: el codegen de BMO C publica solo el PRIMER bloque
 * (`codegen/frame.rs`, `publicar_bloque`), que es el monton de DOOM, pedido
 * antes que esto. */
static int bmo_snd_banco_pedir(void) {
    if (bmo_snd_banco == 0) {
        bmo_snd_banco = (unsigned char *)bmo_bloque_pedir(BMO_SND_BANCO);
        if (bmo_snd_banco == 0) {
            printf("[bmo] sonido: sin memoria para el banco (%d KiB)\n", BMO_SND_BANCO / 1024);
            return 0;
        }
    }
    return 1;
}

static int bmo_snd_abrir(void) {
    unsigned long long bytes;

    bmo_snd_intentos++;
    bmo_snd_cap = bmo_sonido_reclamar();
    if (bmo_snd_cap == 0) {
        return 0; /* lo tiene otro programa; se vuelve a mirar luego */
    }
    if (!bmo_tubo(bmo_snd_cap, BMO_TUBO_ABIERTO, 0)) {
        bmo_sonido_soltar();
        bmo_snd_cap = 0;
        return 0;
    }
    if (!bmo_snd_banco_pedir()) {
        bmo_sonido_soltar();
        bmo_snd_cap = 0;
        return 0;
    }
    bytes = bmo_voz_banco(bmo_snd_cap, bmo_snd_banco);
    if (bytes < BMO_SND_BANCO) {
        printf("[bmo] sonido: el kernel no acepto el banco\n");
        bmo_sonido_soltar();
        bmo_snd_cap = 0;
        return 0;
    }
    if (!bmo_tubo(bmo_snd_cap, BMO_TUBO_ARMAR, 0)) {
        printf("[bmo] sonido: el tubo no se armo\n");
        bmo_sonido_soltar();
        bmo_snd_cap = 0;
        return 0;
    }
    bmo_snd_va = 1;
    printf("[bmo] sonido: VOCES del orquestador, banco de %d KiB, tubo a %d Hz (intento %d)\n",
           BMO_SND_BANCO / 1024,
           (int)bmo_tubo(bmo_snd_cap, BMO_TUBO_FRECUENCIA, 0),
           bmo_snd_intentos);
    /* Prestar un banco calla lo que sonaba del anterior: si habia cancion
     * (la pidio DOOM antes de que el audifono entrara), vuelve a sonar. */
    bmo_musica_arrancar();
    return 1;
}

/* Cada cuantas vueltas de `Update` se vuelve a mirar si ya hay tubo: 18 son
 * dos miradas por segundo. */
#define BMO_SND_MIRAR_CADA 18

/* ===================================================================
 *  5. Los verbos que DOOM llama
 * =================================================================== */

static boolean I_BMO_InitSound(boolean use_sfx_prefix) {
    (void)use_sfx_prefix;
    bmo_snd_va = 0;
    bmo_snd_cap = 0;
    bmo_snd_intentos = 0;
    bmo_snd_espera = 0;
    if (bmo_snd_abrir()) {
        return true;
    }
    /* *** Y SE DEVUELVE `true` AUNQUE NO HAYA TUBO TODAVIA: el modulo sigue
     * vivo y va a seguir mirando. Con `false`, `i_sound.c` deja
     * `sound_module` a nulo y nadie volveria a intentarlo. */
    printf("[bmo] sonido: aun no hay tubo; se sigue mirando mientras juegas\n");
    return true;
}

static void I_BMO_ShutdownSound(void) {
    if (bmo_snd_cap == 0) {
        if (bmo_snd_intentos > 1) {
            printf("[bmo] sonido: nunca hubo tubo (%d miradas)\n", bmo_snd_intentos);
        }
        return;
    }
    bmo_voz_callar(bmo_snd_cap, BMO_VOZ_TODOS);
    bmo_sonido_soltar();
    bmo_snd_cap = 0;
    bmo_snd_va = 0;
}

static int I_BMO_GetSfxLumpNum(sfxinfo_t *sfx) {
    char nombre[9];
    int i;
    const char *n;

    if (sfx->link != 0) {
        sfx = sfx->link;
    }
    n = DEH_String(sfx->name);
    /* DOOM le pone `ds` delante a sus lumps de sonido. */
    nombre[0] = 'd';
    nombre[1] = 's';
    for (i = 0; i < 6 && n[i] != 0; i++) {
        nombre[2 + i] = n[i];
    }
    nombre[2 + i] = 0;
    return W_GetNumForName(nombre);
}

/* `vol` 0..127 y `sep` 0..255 de DOOM a los dos lados de una voz, 0..256.
 * El paneo es LINEAL, como el de 1993: cambiar la ley es cosa de LA MESA. */
static void bmo_snd_lados(int vol, int sep, int *izq, int *der) {
    if (vol < 0) { vol = 0; }
    if (vol > 127) { vol = 127; }
    if (sep < 0) { sep = 0; }
    if (sep > 255) { sep = 255; }
    *izq = vol * (256 - sep) / 127;
    *der = vol * sep / 127;
}

static void I_BMO_UpdateSoundParams(int canal, int vol, int sep) {
    int izq;
    int der;

    if (!bmo_snd_va || canal < 0 || canal >= BMO_SND_CANALES) {
        return;
    }
    bmo_snd_lados(vol, sep, &izq, &der);
    bmo_voz_ajustar(bmo_snd_cap, canal, izq, der);
}

static int I_BMO_StartSound(sfxinfo_t *sfx, int canal, int vol, int sep) {
    int k;
    int izq;
    int der;

    if (!bmo_snd_va || canal < 0 || canal >= BMO_SND_CANALES) {
        return -1;
    }
    if (sfx->lumpnum < 0) {
        sfx->lumpnum = I_BMO_GetSfxLumpNum(sfx);
    }
    k = bmo_snd_al_banco(sfx);
    if (k < 0) {
        return -1;
    }
    bmo_snd_lados(vol, sep, &izq, &der);
    /* ** Y AQUI SE ACABA EL TRABAJO DE DOOM. El orquestador lo toca, lo
     * remuestrea a la frecuencia del aparato y lo suelta cuando acaba. */
    if (!bmo_voz_tocar(bmo_snd_cap, canal, bmo_efecto[k].inicio, bmo_efecto[k].muestras,
                       BMO_VOZ_U8, bmo_efecto[k].hz, izq, der, 0)) {
        return -1;
    }
    return canal;
}

static void I_BMO_StopSound(int canal) {
    if (!bmo_snd_va || canal < 0 || canal >= BMO_SND_CANALES) {
        return;
    }
    bmo_voz_callar(bmo_snd_cap, canal);
}

static boolean I_BMO_SoundIsPlaying(int canal) {
    if (!bmo_snd_va || canal < 0 || canal >= BMO_SND_CANALES) {
        return false;
    }
    return bmo_voz_suena(bmo_snd_cap, canal) != 0;
}

/* **La vuelta del fotograma.** Ya no mezcla nada: solo mira, mientras no lo
 * haya, si el audifono entro. Cuando hay tubo, esto no hace NADA -- y esa es
 * la prueba de que el tiempo ya no es de DOOM. */
static void I_BMO_UpdateSound(void) {
    if (bmo_snd_va) {
        return;
    }
    if (bmo_snd_espera > 0) {
        bmo_snd_espera--;
        return;
    }
    bmo_snd_espera = BMO_SND_MIRAR_CADA;
    (void)bmo_snd_abrir();
}

static void I_BMO_PrecacheSounds(sfxinfo_t *sonidos, int cuantos) {
    /* No hace falta: cada efecto entra al banco la primera vez que suena. */
    (void)sonidos;
    (void)cuantos;
}

static snddevice_t bmo_aparatos[] = {
    SNDDEVICE_SB,
    SNDDEVICE_PAS,
    SNDDEVICE_GUS,
    SNDDEVICE_WAVEBLASTER,
    SNDDEVICE_SOUNDCANVAS,
    SNDDEVICE_AWE32,
};

sound_module_t DG_sound_module = {
    bmo_aparatos,
    sizeof(bmo_aparatos) / sizeof(*bmo_aparatos),
    I_BMO_InitSound,
    I_BMO_ShutdownSound,
    I_BMO_GetSfxLumpNum,
    I_BMO_UpdateSound,
    I_BMO_UpdateSoundParams,
    I_BMO_StartSound,
    I_BMO_StopSound,
    I_BMO_SoundIsPlaying,
    I_BMO_PrecacheSounds,
};

/* ===================================================================
 *  6. La musica: la cancion entera en el banco, tocada en bucle
 * =================================================================== */

#include "bmo_musica.c"
