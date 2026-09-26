/* doomgeneric_bmo.c -- DOOM sobre BMO-X.
 *
 * La capa de plataforma entera: seis funciones y un `main`. Todo lo demas
 * --56.465 lineas-- es DOOM sin tocar una coma.
 *
 * == Por que son solo seis ==
 *
 * `doomgeneric` ya separo el juego de la maquina. Lo que queda de "portar" es
 * contestar seis preguntas: donde pinto, que hora es, duerme, que tecla hay,
 * como me llamo, y arranca. BMO-X tiene las cuatro capabilities que hacen
 * falta desde antes de que existiera este fichero.
 *
 * == Este fichero NO esta en el repo de BMO, y no puede estarlo ==
 *
 * DOOM es GPL-2.0 y el arbol de BMO es Techne. Vive en `BMO-externo/`. Lo que
 * si esta en el repo es todo lo que este fichero LLAMA.
 *
 * == Compilar ==
 *
 *   set BMO_MODS=...\BMO-externo\doom-port\include
 *   bmo-c-front doomgeneric_bmo.c -o doom.bex
 *
 * [!] Con `cwd` en la RAIZ DEL REPO de BMO. `Roots::find` busca la raiz
 * subiendo desde el cwd, y desde el arbol de DOOM no encuentra `tables/`.
 */

/* ** ESTO VA ANTES QUE NADA, y el orden importa de verdad.
 *
 * `<stdlib.h>` trae el monton de Ring 3, y su tamano se declara con este
 * `#define` **antes** de la primera inclusion. La primera la hace
 * `doomgeneric.h` desde dentro de `bmo_unity.c`, asi que si esto fuera detras
 * el monton se quedaria en el 1 MiB por defecto y la zona de DOOM --6 MiB-- no
 * cabria.
 *
 * La cuenta, con lo que DOOM pide de verdad:
 *
 *     zona (I_ZoneBase, DEFAULT_RAM)          6.291.456
 *     DG_ScreenBuffer (640x400x4)             1.024.000
 *     directorio de lumps de doom1.wad          ~121.000
 *     paleta, rutas, I_AtExit y demas             ~8.000
 *     ------------------------------------------------
 *     ~7,3 MiB, y se piden 12 para no ir justo
 *
 * El kernel entrega el bloque **contiguo en fisico**; si no hay hueco lo
 * rechaza y lo dice, en vez de entregarlo a trozos callando. */
#define BMO_MONTON_BYTES (12 * 1024 * 1024)

/* ** EL SONIDO DE DOOM SE ENCIENDE AQUI (2026-09-22).
 *
 * Sin este `#define`, `i_sound.c` compila su lista de modulos como
 * `{ NULL }`, `sound_module` se queda nulo y cada `I_StartSound` es un `if`
 * que no hace nada. DOOM no es que no sonara: es que no lo PEDIA. Ver la
 * cabecera de `bmo_sonido.c`.
 *
 * Va antes de la primera inclusion por lo mismo que `BMO_MONTON_BYTES`: la
 * hace `doomgeneric.h` desde dentro de `bmo_unity.c`. */
#define FEATURE_SOUND 1

/* ** 320x200: EL TAMANO EXACTO DE DOOM, Y LA ESCALA SE HACE AQUI.
 *
 * Esto decia 640x400, y con eso `i_video.c` se encargaba del x2. Suena bien y
 * cuesta tres cosas, las tres medibles:
 *
 * 1. ** LA MITAD DE LA CONVERSION DE PALETA SE TIRABA. Su bucle
 *    (`I_FinishUpdate`) es asi:
 *
 *        while (y--) {
 *            for (i = 0; i < fb_scaling; i++)
 *                cmap_to_fb(line_out, line_in, SCREENWIDTH);   <- MISMA line_in
 *            line_in += SCREENWIDTH;                           <- avanza FUERA
 *        }
 *
 *    O sea que con escala 2 cada fila de origen se convertia **dos veces**,
 *    produciendo dos filas identicas: 128.000 busquedas de paleta por
 *    fotograma donde bastan 64.000. La segunda es una copia de la primera.
 *
 * 2. **La escala quedaba clavada en el binario**, y el panel se conoce en
 *    tiempo de EJECUCION. En 1920x1080, un DOOM de 640x400 es un sello de
 *    correos en medio de la pantalla: ocupa el 12% del area.
 *
 * 3. `DG_ScreenBuffer` media 1 MiB; a 320x200 mide 256 KiB.
 *
 * Con 320x200, `fb_scaling` sale 1 y ese bucle hace **una** pasada por fila,
 * sin bucle interior por pixel. La escala la aplica `DG_DrawFrame`, que es el
 * unico sitio que sabe como es el panel de verdad.
 *
 * [!] Y no se toca una coma de DOOM: son dos numeros de este fichero. */
#define DOOMGENERIC_RESX 320
#define DOOMGENERIC_RESY 200

/* ** LOS DOS AJUSTES QUE VIVIAN EN `i_sdlsound.c`.
 *
 * Con `FEATURE_SOUND` puesto, `I_BindSoundVariables` los ata al fichero de
 * configuracion:
 *
 *     extern int use_libsamplerate;
 *     extern float libsamplerate_scale;
 *
 * ...y los DEFINIA `i_sdlsound.c`, que `unity.py` salta porque arrastra SDL.
 * Van aqui, ANTES del agregado, porque BMO C resuelve los nombres en el orden
 * en que los lee: una definicion detras del uso es "no hay ninguna variable
 * con ese nombre".
 *
 * Valen lo que valian alli, y ninguno de los dos hace nada: el mezclador de
 * `bmo_sonido.c` remuestrea en coma fija y no usa libsamplerate (que tampoco
 * existe aqui). Estan para que el fichero de configuracion de DOOM siga
 * teniendo las mismas lineas que tenia. */
int use_libsamplerate = 0;
float libsamplerate_scale = 0.65f;

#include "bmo_unity.c"

/* ** EL SONIDO, y va DESPUES del agregado a proposito (2026-09-22).
 *
 * `bmo_sonido.c` define `DG_sound_module` y `DG_music_module`, que
 * `i_sound.c` --dentro del agregado-- toma por direccion. La declaracion
 * (`extern`) ya esta en `i_sound.h`, asi que el orden correcto es: primero
 * todo DOOM, y detras el modulo, que necesita `sfxinfo_t`, `W_CacheLumpNum`
 * y `DEH_String` ya declarados.
 *
 * Y `FEATURE_SOUND` se define ANTES de incluir el agregado (mas arriba en
 * este fichero): es lo que hace que la lista de modulos de `i_sound.c` deje
 * de tener un solo `NULL`. */
#include "bmo_sonido.c"

#include <bmo/bmo.h>
#include <bmo/entrada.h>
#include <bmo/pantalla.h>
#include <bmo/superficie.h>
#include "doomkeys.h"
extern int bmo_addline_reves;
extern int bmo_addline_fuera;

/* -- El estado de la plataforma ---------------------------------------- */

pixel_t *DG_ScreenBuffer = 0;

/* == 2026-09-11: DOOM EN UNA VENTANA ======================================
 *
 * Hasta hoy este fichero solo conocia la pantalla EXCLUSIVA: `pantalla.h`, el
 * relevo entero. Por eso DOOM "se ejecutaba sin nada" -- mientras corria no
 * habia escritorio, y al morir la pantalla no volvia a nadie. Y no era un
 * limite de BMO-X: `<bmo/superficie.h>` existe desde agosto y `raycaster_C.c`
 * vive en una ventana desde el 23-08, con teclas y raton por el buzon. DOOM
 * sencillamente no lo pedia.
 *
 * Ahora, si la pantalla NO esta libre, pide una SUPERFICIE. Lanzado desde el
 * escritorio, DOOM dibuja en su propia memoria y el DIRECTOR lo pega en un
 * marco: se cierra con el aspa, se mueve, se cambia con Alt+Tab, y si se
 * cuelga se lleva su ventana y no la maquina. Si la pantalla esta libre
 * --shell de Ring 0, o `presta` desde el escritorio-- se toma entera como
 * siempre, y ese camino no cambia en nada. El orden esta razonado en `DG_Init`.
 *
 * [!] Y hizo falta tocar el COMPILADOR: `WANTS_SCREEN` la deduce BMO C al ver
 * `PANTALLA_RECLAMAR`, y con ella puesta el DIRECTOR se apartaba ANTES de
 * lanzar -- o sea que desde el escritorio DOOM siempre encontraba la pantalla
 * libre y nunca habria ido a la ventana. Desde el 11-09 un programa que ademas
 * sabe componerse (pregunta por su padre) no lleva la bandera. Banco:
 * `tests/bandera_de_pantalla.rs`.
 *
 * ** LO QUE SE PAGA EN VENTANA (L3), dicho aqui y no descubierto jugando:
 *
 *   - la escala es FIJA: x3, 960x600. La superficie se dimensiona al crearla
 *     y no se puede estirar despues, asi que Bloq Despl no hace nada aqui.
 *     x3 y no x4 porque lo que cuesta que el DIRECTOR pegue 576.000 pixeles
 *     uno a uno, 35 veces por segundo, NO ESTA MEDIDO; x4 seria 1.024.000.
 *     El monton no es el limite: la superficie se pide ANTES de la zona de 6
 *     MiB, y x3 son 2,3 MiB (x4, 4,1: cabria con 1,6 de margen).
 *   - una app en ventana NO SABE cuanto mide el panel: REX no lo publica. En
 *     1080p cabe hasta x4 (800 + 29 de titulo contra 1080 - 56); x5 pierde
 *     cinco filas. Elegir la escala por el panel, como hace el camino
 *     exclusivo, aqui no se puede -- y es una pieza que le falta a REX.
 *   - F12 (ceder la pantalla) no aplica: no hay pantalla que ceder. La consola
 *     del kernel se ve al lado, que es la gracia.
 *   - la entrada NO se reclama: las teclas llegan por el buzon de la
 *     superficie cuando el DIRECTOR da el foco a esta ventana. Reclamarla le
 *     quitaria el teclado al escritorio, que es el modelo viejo otra vez.
 *   - en vez de ceder, se DUERME: `bmo_ceder()` en bucle deja al DIRECTOR sin
 *     turno para componer justo estos pixeles (visto en el Ryzen el 19-08).
 *
 * [!] Lo que cuesta componer 960x600 a 35 fps lo dice el metal, no esto
 * (LEY 24): el DIRECTOR pega pixel a pixel, y hasta el 09-09 ese camino tenia
 * el `write_volatile` que el compilador no podia tocar. */
#define VENTANA_ESCALA 3
static BMO_SUPERFICIE *g_sup;           /* la ventana; 0 = pantalla exclusiva */

static unsigned long long g_pantalla;   /* capability del framebuffer */
static unsigned long long g_entrada;    /* capability de raton+teclado */
static unsigned long long g_fb;         /* donde empiezan los pixeles */
static int g_ancho;
static int g_alto;
static int g_paso;                      /* stride EN PIXELES, no en bytes */
static int g_x0;                        /* esquina donde cae el fotograma */
static int g_y0;

/* -- La escala, que ahora es de este fichero ----------------------------
 *
 * `g_escala` multiplica los 320x200 de DOOM. Se elige en `DG_Init` a partir
 * del panel REAL y se puede cambiar en caliente con Bloq Despl (ver
 * `DG_GetKey`), que es lo que convierte un arranque en una medida de todas
 * las escalas en vez de una sola. */
static int g_escala;
static int g_escala_max;
static int g_dst_ancho;                 /* 320 * escala, ya recortado */
static int g_dst_alto;                  /* 200 * escala, ya recortado */

/* Una fila destino ya expandida, EN RAM NORMAL.
 *
 * ** Y esto es lo unico delicado del blit. Las `escala` filas de salida son
 * identicas, asi que la segunda podria copiarse de la primera... pero la
 * primera ya esta en el framebuffer, que es memoria WRITE-COMBINING: escribir
 * ahi va rapido y **LEER va lentisimo**, porque no hay cache que valga. Copiar
 * de la pantalla a la pantalla seria el camino obvio y el peor de todos.
 *
 * Con la fila en RAM se expande UNA vez y se vuelca `escala` veces desde
 * memoria cacheada. El tope de 4096 es lo que fija `ESCALA_TOPE`. */
#define ANCHO_TOPE 4096
#define ESCALA_TOPE (ANCHO_TOPE / DOOMGENERIC_RESX)
static unsigned int g_fila[ANCHO_TOPE];
static unsigned long long g_tsc0;       /* el TSC del arranque */
static unsigned long long g_tsc_khz;    /* ciclos por milisegundo */

/* -- El metro (seccion 3) ----------------------------------------------
 *
 * Todo esto lo escriben `DG_DrawFrame` y `DG_GetKey` y lo lee `medir`. Se
 * declara aqui arriba porque el blit se mide antes de que exista el metro. */
/* Pestillo del aviso de "no dibujo": canta una vez. Ver `DG_DrawFrame`. */
static int g_mudo_dicho;
/// Fotogramas dados desde el arranque. Solo para `[vivo]`, ver `main`.
static int g_latido;
static unsigned long long g_med_blit;       /* ciclos dentro del blit */
/* == INSTRUMENTO (2026-09-04): LAS DOS MITADES DEL BLIT ==================
 *
 * `c/blit.bex` midio el framebuffer: 3.140 ciclos/KiB, o sea 1,47 GB/s. Los
 * 6,4 MB de un fotograma a escala 5 deberian costar 4,36 ms, y el blit mide
 * 25,49. **El 83% no es la copia** -- y esto dice de quien es.
 *
 *    g_med_exp     expandir la fila en RAM   (320 x escala escrituras)
 *    g_med_vol     volcarla al framebuffer   (escala memcpy)
 *
 * Se quita cuando conteste. */
static unsigned long long g_med_exp;
static unsigned long long g_med_vol;
static unsigned long long g_med_blit_total; /* lo mismo, sin reiniciar nunca */
static unsigned long long g_med_t0;         /* TSC de la ultima linea */
static unsigned long long g_med_arranque;   /* TSC de la primera */
static int g_med_fotogramas;                /* desde la ultima linea */
static int g_med_total;                     /* desde el principio */
static int g_med_tic0;                      /* `gametic` en la ultima linea */
static int g_med_crudas;                    /* eventos de tecla leidos */
static int g_med_teclas;                    /* los que DOOM entiende */

/* Del par (panel, escala) salen el tamano y la esquina. Es una funcion y no
 * cuatro lineas dentro de `DG_Init` porque la escala cambia en caliente, y dos
 * copias de este calculo se separarian el dia que una de las dos se toque. */
static void geometria(void)
{
    g_dst_ancho = DOOMGENERIC_RESX * g_escala;
    g_dst_alto = DOOMGENERIC_RESY * g_escala;
    /* Si no cabe --panel raro, o escala forzada a 1 en un panel diminuto-- se
     * recorta aqui, y el blit no tiene que volver a pensarlo. */
    if (g_dst_ancho > g_ancho) { g_dst_ancho = g_ancho; }
    if (g_dst_alto > g_alto) { g_dst_alto = g_alto; }
    g_x0 = (g_ancho - g_dst_ancho) / 2;
    g_y0 = (g_alto - g_dst_alto) / 2;
    if (g_x0 < 0) { g_x0 = 0; }
    if (g_y0 < 0) { g_y0 = 0; }
}

/* ** BLOQ DESPL CAMBIA LA ESCALA EN CALIENTE, y no es un lujo.
 *
 * La escala mas grande es la mas bonita y la mas cara: pasar de x2 a x5 son
 * 6,25 veces mas pixeles que escribir por fotograma. Cual sale a cuenta no se
 * puede razonar desde aqui -- depende del ancho de banda real hacia el
 * framebuffer de esa GPU por ese PCIe, que es justo lo que ningun anfitrion
 * puede medir.
 *
 * Con esta tecla, **un arranque mide todas las escalas** en vez de una: se
 * juega un rato en cada una y el metro imprime una linea por cada una, con su
 * numero de escala delante. Sin ella harian falta tantos flasheos como
 * escalas.
 *
 * Se eligio Bloq Despl porque es la unica tecla que produce scancode y que
 * DOOM **no usa para nada**. Se come aqui: DOOM no llega a verla. */
static void limpiar_pantalla(void)
{
    int y;
    unsigned int *destino;

    /* Al encoger, lo de fuera del cuadro nuevo se quedaria con la imagen
     * grande alrededor -- un marco de restos que parece un fallo de dibujo. */
    for (y = 0; y < g_alto; y = y + 1) {
        destino = (unsigned int *)(g_fb + (unsigned long long)(y * g_paso) * 4);
        memset(destino, 0, g_ancho * 4);
    }
}

/* ** CEDER LA PANTALLA PARA MIRAR LA CONSOLA, Y VOLVER (F12).
 *
 * El equivalente del `Ctrl+Alt+F1` de Linux, y por el mismo motivo: mientras
 * DOOM repinta 28 veces por segundo, **nada que escriba el kernel dentro de su
 * rectangulo sobrevive 35 ms**. No se ve el `[perf]`, ni el `[heap]`, ni un
 * aviso del kernel -- no porque no se impriman, sino porque se tapan.
 *
 * Cediendo, DOOM se queda CIEGO pero VIVO: la pantalla se para en el ultimo
 * fotograma y ahi se puede leer, o pulsar `F11` para volcar el klog encima.
 * Otra F12 y vuelve.
 *
 * [!] `F12` porque en un jugador no hace nada --su uso original es la camara
 * espia de una partida en red-- y porque es la tecla que el compositor ya usa
 * para su consola, asi que el dedo va solo. Se come aqui: DOOM no la ve.
 *
 * Las dos operaciones son de `<bmo/pantalla.h>` y salieron el 2026-09-01:
 * hasta ese dia esto no se podia escribir, porque `PANTALLA_SOLTAR` existia en
 * el kernel y **no estaba publicada**. */
static void ceder_o_volver(void)
{
    if (g_pantalla != 0) {
        if (bmo_pantalla_soltar() == 0) {
            g_pantalla = 0;
            g_fb = 0;
            printf("DOOM: pantalla CEDIDA. F11 vuelca el klog; F12 vuelve.\n");
        } else {
            printf("DOOM: no se pudo ceder la pantalla\n");
        }
        return;
    }
    {
        BMO_PANTALLA p;
        if (bmo_pantalla_abrir(&p) == 0) {
            printf("DOOM: no se pudo recuperar la pantalla (la tiene otro)\n");
            return;
        }
        /* Se REMIDE todo: entre ceder y volver el panel pudo cambiar de manos,
         * y una anchura vieja pinta en el sitio que no es. */
        g_pantalla = p.cap;
        g_fb = (unsigned long long)p.pixeles;
        g_ancho = p.ancho;
        g_alto = p.alto;
        g_paso = p.paso;
        geometria();
        limpiar_pantalla();
        printf("DOOM: pantalla recuperada, %d x %d\n", g_ancho, g_alto);
    }
}

static void cambiar_escala(void)
{
    g_escala = g_escala + 1;
    if (g_escala > g_escala_max) {
        g_escala = 1;
    }
    geometria();
    limpiar_pantalla();
    printf("DOOM: escala x%d -> %d x %d\n", g_escala, g_dst_ancho, g_dst_alto);
}

/* -- 2.0  DG_Init: reclamar pantalla y entrada -------------------------- */

/* ** AQUI HABIA TRES NUMEROS DEL KERNEL COPIADOS A MANO, y se fueron el
 * 2026-09-01, cuando por fin hubo donde vivieran: `<bmo/pantalla.h>`.
 *
 * REX no publicaba el framebuffer --daba `PANTALLA_RECLAMAR` y no las cuatro
 * operaciones que se hacen CON el handle-- asi que este fichero y
 * `raycaster_C.c` tenian cada uno su copia de `0x01`, `0x02` y `0x03`. Dos
 * copias de un numero del kernel que nadie comparaba con el original.
 *
 * [!] Y de paso llega `bmo_pantalla_cabe`, que es la comprobacion de indice
 * hecha UNA vez y contra los bytes que el kernel mapeo de verdad. La fila
 * 200 salio de tener ese `if` copiado en dos sitios y mal en los dos. */

void DG_Init()
{
    int escala_a;
    int escala_b;
    BMO_PANTALLA pan;
    unsigned long long hz;

    /* ** PRIMERO SE PREGUNTA POR LA PANTALLA, Y SOLO SI ESTA LIBRE SE TOMA.
     *
     * Es el orden contrario al del raycaster, y es a proposito. Tres formas de
     * llegar aqui, y la pantalla contesta distinto en cada una:
     *
     *    icono o `run` del escritorio   el DIRECTOR la tiene -> NO  -> ventana
     *    `presta apps/doom.bex`         el DIRECTOR la solto -> SI  -> entera
     *    shell de Ring 0                nadie la tiene       -> SI  -> entera
     *
     * Asi `presta` sigue siendo "a pantalla completa, a proposito" --con x5,
     * Bloq Despl y F12--, y el lanzamiento normal es la ventana. Si se pidiera
     * la ventana primero, `presta` se quedaria treinta segundos a oscuras
     * esperando una reclamacion que no llega. Ver "DOOM EN UNA VENTANA". */
    if (bmo_pantalla_abrir(&pan) == 0) {
        /* 64 ranuras de buzon, las mismas que el raycaster. */
        g_sup = bmo_superficie_crear_con_buzon(DOOMGENERIC_RESX * VENTANA_ESCALA,
                                               DOOMGENERIC_RESY * VENTANA_ESCALA, 64);
        if (g_sup == 0) {
            /* *** TRES CAUSAS, Y HASTA EL 12-09 ESTE MENSAJE SOLO CONTABA DOS.
             *
             * `crear_con_buzon` devuelve 0 si no hay monton, si no hay padre a
             * quien ofrecer, o --desde hoy-- si el padre RECHAZO la oferta. Ese
             * tercer caso se tragaba antes: la ventana no salia y nada lo decia.
             * El kernel SI apunta el motivo, asi que se manda a buscarlo en vez
             * de dejar al que mira adivinando. */
            printf("DOOM: sin ventana. Monton, padre, o el padre la RECHAZO\n");
            printf("DOOM: el motivo, en `cabina fallos` -- linea `prestamo`\n");
            return;
        }
        g_fb = (unsigned long long)bmo_superficie_pixeles(g_sup);
        g_ancho = DOOMGENERIC_RESX * VENTANA_ESCALA;
        g_alto = DOOMGENERIC_RESY * VENTANA_ESCALA;
        g_paso = g_ancho;               /* sin relleno: stride = ancho */
        g_escala_max = VENTANA_ESCALA;
        g_escala = VENTANA_ESCALA;
        geometria();
        /* La memoria del monton no viene a cero, y el DIRECTOR pega lo que
         * haya en cuanto suba la secuencia. */
        limpiar_pantalla();
        printf("DOOM: en una ventana de %d x %d (escala x%d, fija)\n",
               g_ancho, g_alto, g_escala);
        if (DG_ScreenBuffer == 0) {
            printf("DOOM: NO HAY BUFFER (malloc de %d bytes fallo)\n",
                   DOOMGENERIC_RESX * DOOMGENERIC_RESY * 4);
        }
        hz = bmo_info(BMO_INFO_TSC_HZ);
        g_tsc_khz = hz / 1000;
        if (g_tsc_khz == 0) { g_tsc_khz = 1; }
        g_tsc0 = __rdtsc();
        return;
    }

    /* La pantalla EXCLUSIVA, que estaba libre: el camino de siempre. */
    g_pantalla = pan.cap;
    g_fb = (unsigned long long)pan.pixeles;
    g_ancho = pan.ancho;
    g_alto = pan.alto;
    g_paso = pan.paso;

    /* ** La escala mas grande que CABE ENTERA, y se decide aqui porque hasta
     * aqui no se sabia como es el panel.
     *
     * Entera y no fraccionaria a proposito: un factor no entero obliga a
     * inventarse pixeles --interpolar-- y DOOM se dibujo pixel a pixel. Un x5
     * es DOOM cinco veces mas grande; un x5,4 es DOOM borroso.
     *
     * En 1920x1080 sale 5: 1600x1000, o sea el 77% de la pantalla contra el
     * 12% que ocupaba antes.
     *
     * [!] Lo que NO corrige, y hay que decirlo: los 320x200 originales se
     * veian en un monitor 4:3, o sea con pixeles MAS ALTOS que anchos. Con
     * pixeles cuadrados todo sale un poco achatado. Ya pasaba con el 640x400,
     * asi que esto no lo empeora -- pero tampoco lo arregla, y el arreglo
     * seria escalar distinto en cada eje. */
    escala_a = g_ancho / DOOMGENERIC_RESX;
    escala_b = g_alto / DOOMGENERIC_RESY;
    g_escala_max = escala_a;
    if (escala_b < g_escala_max) { g_escala_max = escala_b; }
    if (g_escala_max > ESCALA_TOPE) { g_escala_max = ESCALA_TOPE; }
    /* Un panel mas pequeno que 320x200 no existe hoy, pero si existiera la
     * escala 0 dejaria la pantalla negra sin decir por que: se fuerza a 1 y el
     * recorte lo hace el blit. */
    if (g_escala_max < 1) { g_escala_max = 1; }
    g_escala = g_escala_max;
    geometria();

    /* ** EL BORRADO QUE ESTE FICHERO DABA POR HECHO, y no ocurria.
     *
     * `limpiar_pantalla` estaba escrita, probada y llamada desde DOS sitios
     * --Bloq Despl y el regreso de F12-- y NO desde el arranque, que es el
     * unico sitio donde la pantalla trae dibujo de OTRO. El comentario de
     * cincuenta lineas mas abajo lleva desde el 09-01 diciendo *"DG_Init
     * reclama la pantalla y la LIMPIA"*: describia una llamada que no existia.
     *
     * *** Y se veia en cada partida. DOOM ocupa el 77% del panel a escala x5;
     * el 23% de alrededor **no lo toca nadie**, asi que el escritorio se
     * quedaba debajo -- la caja de Ejecutar con su `doom.bex` y la rejilla de
     * iconos, alrededor del juego, hasta reiniciar. Parecia que el compositor
     * no limpiaba el fondo, y el compositor no puede: la pantalla ya no es
     * suya. El que la reclama es el que la deja como quiere encontrarla.
     *
     * Va ANTES del printf de abajo a proposito, y eso ya estaba razonado en el
     * bloque del buffer: la consola del kernel escribe en ESTOS pixeles, asi
     * que lo impreso antes del borrado se borra con lo demas. */
    limpiar_pantalla();

    printf("DOOM: panel %d x %d, escala x%d -> %d x %d (max x%d)\n",
           g_ancho, g_alto, g_escala, g_dst_ancho, g_dst_alto, g_escala_max);

    /* La entrada, tambien exclusiva. Sin ella DOOM arranca y se ve, pero no
     * responde -- y eso hay que DECIRLO, porque desde fuera se parece a un
     * cuelgue. */
    g_entrada = bmo_valor(BMO_TAREA_ACTUAL, BMO_OP_ENTRADA_RECLAMAR, 0, 0, 0);
    if (g_entrada == 0) {
        printf("DOOM: sin teclado (la entrada es de otro proceso)\n");
    }

    /* El reloj sale del TSC y no de `INFO_TICKS`, y es a proposito: el tick del
     * LAPIC se calibra en el arranque y su frecuencia no esta declarada en
     * ninguna constante que este programa pueda leer. El TSC si -- su
     * frecuencia la mide el kernel y la publica. */
    /* -- ** EL BUFFER QUE NADIE COMPROBO, y es la pantalla vacia -------
     *
     * `doomgeneric_Create` hace esto, y es DOOM sin tocar:
     *
     *     DG_ScreenBuffer = malloc(DOOMGENERIC_RESX * DOOMGENERIC_RESY * 4);
     *     DG_Init();
     *
     * 256.000 bytes **sin mirar el retorno**. Y si sale 0 --el monton pide 12
     * MiB CONTIGUOS EN FISICO y el kernel puede decir que no-- pasa esto:
     *
     *     DG_Init         reclama la pantalla y la LIMPIA
     *     DG_DrawFrame    `if (DG_ScreenBuffer == 0) return;` en cada fotograma
     *
     * *** O sea que DOOM corre el juego ENTERO, invisible, y la pantalla se
     * queda como la dejo el borrado: vacia. Que es exactamente el sintoma que
     * lleva repitiendose, y no tenia ni una linea que lo dijera.
     *
     * [!] Y se dice AQUI, despues de `limpiar_pantalla`, a proposito: la
     * consola del kernel escribe en LOS MISMOS PIXELES que esta app --lo dice
     * `fb::claim`, *"no hay copia ni doble bufer"*-- asi que un mensaje impreso
     * antes del borrado se borra con todo lo demas. */
    if (DG_ScreenBuffer == 0) {
        printf("DOOM: NO HAY BUFFER (malloc de %d bytes fallo)\n",
               DOOMGENERIC_RESX * DOOMGENERIC_RESY * 4);
        printf("DOOM: el monton pide %d MiB contiguos; si el kernel dijo que no,\n",
               BMO_MONTON_BYTES / (1024 * 1024));
        printf("DOOM: no se va a dibujar NADA y la pantalla se queda vacia.\n");
    }

    hz = bmo_info(BMO_INFO_TSC_HZ);
    g_tsc_khz = hz / 1000;
    if (g_tsc_khz == 0) { g_tsc_khz = 1; }
    g_tsc0 = __rdtsc();
}

/* == 2026-09-04: LA FILA SIN ESTIRAR PIXEL A PIXEL =======================
 *
 * El bucle de antes escribia UN pixel por vuelta:
 *
 *     for (x ...) { p = origen[y * 320 + x];
 *                   for (k = 0; k < escala; k++) fila[x * escala + k] = p; }
 *
 * A escala 5 eso son 1.600 escrituras de 4 bytes por fila y 320.000 por
 * fotograma, y cada una paga un `imul` para el indice, una lectura de la
 * global `g_escala` y tres accesos a la pila. **El trabajo util es copiar un
 * numero; todo lo demas es la aritmetica de saber DONDE.**
 *
 * Aqui no hay indices: hay dos punteros que caminan. Y como un pixel son 4
 * bytes, DOS pixeles iguales caben en UNA escritura de 8 -- que es el ancho
 * natural de esta maquina. Sabia escribir dos de golpe y se le estaba pidiendo
 * de uno en uno.
 *
 *     escala 5  ->  2 escrituras de 8 + 1 de 4  =  3, no 5
 *     escala 4  ->  2 de 8                      =  2, no 4
 *     escala 2  ->  1 de 8                      =  1, no 2
 *
 * A escala 5 son 192.000 escrituras por fotograma en vez de 320.000, y ninguna
 * paga `imul` ni recarga de global.
 *
 * [!] LA MASCARA NO SOBRA. `p` es `unsigned int`. Si el ensanchamiento a 64
 * bits llevara signo, un pixel con el bit alto puesto --y los de DOOM lo
 * tienen, el alfa es 0xFF-- llenaria de unos la mitad de arriba y el segundo
 * pixel del par saldria blanco. El ANCHO DE UN ENSANCHAMIENTO es exactamente
 * la clase de fallo que este mes se ha pagado cinco veces seguidas, asi que se
 * escribe explicito, se construye sin literales de 32 bits, y hay una casilla
 * del censo que lo EJECUTA en el emulador antes de que el Ryzen lo vea.
 *
 * [!] Las escalas impares dejan la fila desalineada cada 4 bytes. A `g_fila`,
 * que es RAM normal, en un Zen 3 eso no cuesta nada; hacia el framebuffer si
 * costaria, y por eso la expansion vive AQUI y el volcado sigue siendo un
 * `memcpy` de fila entera --o sea `rep movsb`, que `c/blit.bex` midio como lo
 * mejor que hay hacia memoria write-combining--. */
static void expandir_fila(unsigned int *destino, unsigned int *fuente,
                          int pixeles, int escala)
{
    unsigned long long *d8;
    unsigned long long par;
    unsigned long long mascara;
    unsigned int *d4;
    unsigned int p;
    int dobles;
    int suelto;
    int x;
    int j;

    if (escala == 1) {
        memcpy(destino, fuente, pixeles * 4);
        return;
    }

    /* 0x00000000FFFFFFFF construido a mano: un literal de 32 bits es
     * justamente lo que podria llegar ensanchado con signo. */
    mascara = 1;
    mascara = (mascara << 32) - 1;

    dobles = escala / 2;            /* escrituras de 8 bytes por pixel */
    suelto = escala - dobles * 2;   /* 0 o 1 escritura de 4 que sobra */

    d4 = destino;
    for (x = 0; x < pixeles; x = x + 1) {
        p = *fuente;
        fuente = fuente + 1;

        par = (unsigned long long)p;
        par = par & mascara;
        par = par | (par << 32);

        d8 = (unsigned long long *)d4;
        j = dobles;
        while (j > 0) {
            *d8 = par;
            d8 = d8 + 1;
            j = j - 1;
        }
        d4 = (unsigned int *)d8;

        if (suelto != 0) {
            *d4 = p;
            d4 = d4 + 1;
        }
    }
}

/* -- 2.1  DG_DrawFrame: el fotograma a la pantalla ---------------------- */

/* DOOM entrego 320x200 pixeles de 32 bits en `DG_ScreenBuffer`: la conversion
 * por paleta la hizo `cmap_to_fb` dentro de `I_FinishUpdate`. Aqui se AGRANDAN
 * y se ponen donde se ven.
 *
 * Fila a fila y no de una vez porque el framebuffer tiene STRIDE: la fila
 * siguiente empieza `g_paso` pixeles despues, que no tiene por que ser
 * `g_ancho`. Copiar de corrido con un solo `memcpy` funciona en el panel donde
 * coinciden y sale torcido en el primero donde no.
 *
 * ** LA FORMA DEL BUCLE, que es donde esta el trabajo:
 *
 *   por cada fila de DOOM (200):
 *       expandirla UNA vez a `g_fila`, en RAM        <- 320 lecturas
 *       volcarla `escala` veces al framebuffer       <- `escala` memcpy
 *
 * Quien expande es `expandir_fila`, justo arriba, y desde el 04-09 no va de
 * pixel en pixel: ver alli por que.
 *
 * Cada pixel de origen se lee **una sola vez** por fotograma, y todo lo que
 * toca la pantalla es un `memcpy` de una fila entera contigua -- que en BMO C
 * es `rep movsb`, o sea lo mejor que sabe hacer esta maquina hacia memoria
 * write-combining. Expandir directamente sobre el framebuffer, pixel a pixel,
 * seria el mismo numero de escrituras pero de a cuatro bytes y sin ninguna
 * fila que reaprovechar. */
void DG_DrawFrame()
{
    int y;
    int k;
    int filas;
    unsigned int *destino;
    unsigned int *origen;
    unsigned long long t0;
    unsigned long long t_mitad;

    /* ** ESTA GUARDA VOLVIA SIN DECIR NADA, y es la mitad del sintoma.
     *
     * Un fotograma que no se dibuja y no se queja es indistinguible de un
     * fotograma que se dibuja en negro. Con la pantalla ya reclamada y limpia,
     * las dos cosas se ven igual: vacio.
     *
     * *** Es el mismo fallo que este dia ya encontro dos veces en otros sitios
     * --un guardian que no encuentra nada y pasa en verde, un instrumento que
     * no corre y suena igual que uno que no ve nada--. Aqui, ademas, tapaba la
     * unica salida que tiene el programa.
     *
     * Con PESTILLO: canta una vez y se calla. Un aviso por fotograma a 35
     * fotogramas por segundo tapa su propio mensaje, que es la leccion del
     * cepo del 30-08 aplicada sin volver a aprenderla. */
    if ((g_pantalla == 0 && g_sup == 0) || DG_ScreenBuffer == 0) {
        if (g_mudo_dicho == 0) {
            g_mudo_dicho = 1;
            if (g_pantalla == 0 && g_sup == 0) {
                printf("DOOM: NO DIBUJO -- no tengo la pantalla (la tiene otro)\n");
            } else {
                printf("DOOM: NO DIBUJO -- tengo la pantalla y NO hay buffer\n");
            }
        }
        return;
    }
    t0 = __rdtsc();

    origen = (unsigned int *)DG_ScreenBuffer;
    for (y = 0; y < DOOMGENERIC_RESY; y = y + 1) {
        /* Las filas de destino que caben. En la ultima fila de un panel que no
         * es multiplo exacto, esto es lo que evita escribir fuera. */
        filas = g_escala;
        if (y * g_escala + filas > g_dst_alto) {
            filas = g_dst_alto - y * g_escala;
        }
        if (filas <= 0) {
            break;
        }

        if (g_escala == 1) {
            /* Camino directo: no hay nada que expandir. */
            destino = (unsigned int *)(g_fb
                + (unsigned long long)((g_y0 + y) * g_paso + g_x0) * 4);
            memcpy(destino, origen + y * DOOMGENERIC_RESX, g_dst_ancho * 4);
            continue;
        }

        t_mitad = __rdtsc();
        expandir_fila(g_fila, origen + y * DOOMGENERIC_RESX,
                      DOOMGENERIC_RESX, g_escala);
        g_med_exp = g_med_exp + (__rdtsc() - t_mitad);

        t_mitad = __rdtsc();
        for (k = 0; k < filas; k = k + 1) {
            destino = (unsigned int *)(g_fb
                + (unsigned long long)((g_y0 + y * g_escala + k) * g_paso + g_x0) * 4);
            memcpy(destino, g_fila, g_dst_ancho * 4);
        }
        g_med_vol = g_med_vol + (__rdtsc() - t_mitad);
    }

    /* El metro. Ver la seccion 3: lo que cuesta el blit se mide aqui porque es
     * el unico sitio donde se sabe donde empieza y donde acaba. */
    t0 = __rdtsc() - t0;
    g_med_blit = g_med_blit + t0;
    g_med_blit_total = g_med_blit_total + t0;
    g_med_fotogramas = g_med_fotogramas + 1;

    /* En ventana, el dibujo esta ENTERO ahora y no antes: es lo unico que hace
     * que el DIRECTOR lo pegue. Ver la secuencia en `<bmo/superficie.h>`. */
    if (g_sup != 0) {
        bmo_superficie_lista(g_sup);
    }
}

/* -- 2.2  DG_GetTicksMs ------------------------------------------------- */

/* Milisegundos desde que arranco DOOM.
 *
 * Del TSC, dividido por los ciclos que caben en un milisegundo. La resta va
 * ANTES de la division: dividir primero perderia la parte baja de los dos
 * numeros y el reloj avanzaria a saltos de milisegundos enteros mal
 * redondeados. */
uint32_t DG_GetTicksMs()
{
    unsigned long long ahora;
    uint32_t ms;
    if (g_tsc_khz == 0) {
        return 0;
    }
    ahora = __rdtsc();
    ms = (uint32_t)((ahora - g_tsc0) / g_tsc_khz);
    /* Aqui el 22-09 se colgo el relleno del sonido, para que DOOM rellenara
     * aunque estuviera derritiendo la pantalla. Ya no hace falta: el sonido lo
     * toca el ORQUESTADOR (las voces, `bmo_sonido.c`) y el tiempo no es de
     * DOOM. Un reloj que solo da la hora otra vez. */
    return ms;
}

/* -- 2.3  DG_SleepMs ---------------------------------------------------- */

/* Esperar DORMIDO, no cediendo ni girando (2026-09-11).
 *
 * Hasta hoy esto era un bucle sobre el TSC con `bmo_ceder()` dentro, con este
 * argumento: *"BMO-X no tiene 'duerme hasta el instante T': lo que hay es
 * ceder"*. Ya lo tiene -- `bmo_dormir` es `WAIT` con plazo, y existe desde
 * agosto por el raycaster en ventana -- y el bucle de ceder era el peor de los
 * dos mundos: `bmo_ceder()` vuelve a la cola LISTO, asi que DOOM esperando su
 * siguiente tic se comia **el 100 % de un nucleo** para no hacer nada. DOOM
 * llama a esto entre tic y tic (35 por segundo): la mayor parte del tiempo de
 * una partida en el menu es ESTA funcion.
 *
 * Con `WAIT` la tarea queda BLOQUEADA y el nucleo se va a la tarea idle, que
 * desde hoy duerme hondo (`PLAN_VATIOS.md`, W1 y W5). Y el teclado no se
 * apaga: el xHC se sondea en `DG_GetKey`, que DOOM llama en cada tic igual.
 *
 * [!] La granularidad es el tick del kernel: 1 ms pedido puede ser 2. Para
 * un juego a 35 Hz no se nota; para medirlo esta el metro de la seccion 3. */
void DG_SleepMs(uint32_t ms)
{
    if (ms == 0) {
        bmo_ceder();
        return;
    }
    bmo_dormir((unsigned long long)ms * 1000000ULL);
}

/* -- 2.4  DG_GetKey: el scancode a la tecla de DOOM --------------------- */

/* La tabla, indexada por scancode Set 1.
 *
 * ** Se traduce de SCANCODE y no de caracter, y esa es la pieza que hizo falta
 * escribir en el kernel para que esto existiera. Un caracter no tiene
 * "soltar", asi que quien echa a andar no para nunca; y Shift, Ctrl y Alt --en
 * DOOM correr, disparar y strafe-- no producen caracter ninguno.
 *
 * Los codigos de la derecha son los de `doomkeys.h`. Los que valen 0 son
 * teclas que ni DOOM ni el teclado producen.
 *
 * ** EL TECLADO VA ENTERO, y no es capricho. La tabla vieja tenia treinta
 * teclas, las justas para andar y disparar, y con eso NO SE JUEGA:
 *
 *   - los trucos son letras -- `iddqd`, `idkfa`, `idclev`, `idbehold`; sin la
 *     `k`, la `f` o la `b` no hay ninguno,
 *   - guardar una partida pide ESCRIBIR SU NOMBRE, o sea el alfabeto entero,
 *   - F2/F3 guardan y cargan, F6/F9 son la partida rapida, F10 sale,
 *   - `+` y `-` cambian el tamano de la ventana del juego,
 *   - y Pausa es Pausa.
 *
 * O sea que la diferencia entre "se mueve" y "se juega" era una tabla. */
static unsigned char g_tabla[128] = {
    [BMO_SC_ESC] = KEY_ESCAPE,
    [BMO_SC_1] = '1', [BMO_SC_2] = '2', [BMO_SC_3] = '3', [BMO_SC_4] = '4',
    [BMO_SC_5] = '5', [BMO_SC_6] = '6', [BMO_SC_7] = '7', [BMO_SC_8] = '8',
    [BMO_SC_9] = '9', [BMO_SC_0] = '0',
    /* `-` y `=` encogen y agrandan la ventana del juego. */
    [BMO_SC_MENOS] = KEY_MINUS, [BMO_SC_IGUAL] = KEY_EQUALS,
    [BMO_SC_RETROCESO] = KEY_BACKSPACE,
    [BMO_SC_TAB] = KEY_TAB,          /* el mapa */
    [BMO_SC_ENTRAR] = KEY_ENTER,
    [BMO_SC_ESPACIO] = ' ',          /* usar: `key_use` vale ' ' */
    /* El alfabeto, en minusculas: es lo que comparan los trucos y lo que
     * escribe el nombre de una partida guardada. */
    [BMO_SC_A] = 'a', [BMO_SC_B] = 'b', [BMO_SC_C] = 'c', [BMO_SC_D] = 'd',
    [BMO_SC_E] = 'e', [BMO_SC_F] = 'f', [BMO_SC_G] = 'g', [BMO_SC_H] = 'h',
    [BMO_SC_I] = 'i', [BMO_SC_J] = 'j', [BMO_SC_K] = 'k', [BMO_SC_L] = 'l',
    [BMO_SC_M] = 'm', [BMO_SC_N] = 'n', [BMO_SC_O] = 'o', [BMO_SC_P] = 'p',
    [BMO_SC_Q] = 'q', [BMO_SC_R] = 'r', [BMO_SC_S] = 's', [BMO_SC_T] = 't',
    [BMO_SC_U] = 'u', [BMO_SC_V] = 'v', [BMO_SC_W] = 'w', [BMO_SC_X] = 'x',
    [BMO_SC_Y] = 'y', [BMO_SC_Z] = 'z',
    [BMO_SC_COMA] = ',', [BMO_SC_PUNTO] = '.', [BMO_SC_BARRA] = '/',
    [BMO_SC_PUNTO_Y_COMA] = ';', [BMO_SC_APOSTROFE] = '\'',
    [BMO_SC_CORCHETE_IZQ] = '[', [BMO_SC_CORCHETE_DER] = ']',
    [BMO_SC_BARRA_INV] = '\\', [BMO_SC_ACENTO_GRAVE] = '`',
    /* Disparar y correr. Los dos son la version DERECHA a proposito: son las
     * que `m_controls.c` trae por defecto (`key_fire = KEY_RCTRL`,
     * `key_speed = KEY_RSHIFT`), y el driver da el mismo scancode para las dos
     * mitades del Ctrl. */
    [BMO_SC_CTRL] = KEY_RCTRL,
    [BMO_SC_MAYUS_IZQ] = KEY_RSHIFT,
    [BMO_SC_MAYUS_DER] = KEY_RSHIFT,
    /* Strafe. Las dos mitades del Alt, porque en un teclado espanol la derecha
     * es AltGr y sale con codigo propio. */
    [BMO_SC_ALT] = KEY_RALT,
    [BMO_SC_ALTGR] = KEY_RALT,
    [BMO_SC_BLOQ_MAYUS] = KEY_CAPSLOCK,
    /* Las de funcion: ayuda, detalle, guardar, cargar, volumen, gamma, salir.
     * F11 y F12 se dan igual aunque el compositor use F12 para su consola --
     * mientras DOOM tiene la entrada, el compositor no ve una sola tecla. */
    [BMO_SC_F1] = KEY_F1, [BMO_SC_F2] = KEY_F2, [BMO_SC_F3] = KEY_F3,
    [BMO_SC_F4] = KEY_F4, [BMO_SC_F5] = KEY_F5, [BMO_SC_F6] = KEY_F6,
    [BMO_SC_F7] = KEY_F7, [BMO_SC_F8] = KEY_F8, [BMO_SC_F9] = KEY_F9,
    [BMO_SC_F10] = KEY_F10, [BMO_SC_F11] = KEY_F11,
    /* [!] `BMO_SC_F12` YA NO se entrega: se la queda `DG_GetKey` para ceder la
     * pantalla (ver `ceder_o_volver`). Su uso en DOOM es la camara espia de una
     * partida en red, que aqui no existe. */
    /* [!] `BMO_SC_PAUSA` y `BMO_SC_BLOQ_NUM` son EL MISMO BYTE (ver
     * `<bmo/entrada.h>`), asi que Bloq Num tambien pausa. Se elige Pausa
     * porque es la que un juego usa; Bloq Num aqui no hace nada. */
    [BMO_SC_PAUSA] = KEY_PAUSE,
    /* [!] `BMO_SC_BLOQ_DESPL` NO esta aqui a proposito: se la queda
     * `DG_GetKey` para cambiar la escala y DOOM no la ve. Si algun dia hace
     * falta como `KEY_SCRLCK`, hay que quitarla de alli primero. */
    /* Flechas: el movimiento original de 1993. */
    [BMO_SC_ARRIBA] = KEY_UPARROW,
    [BMO_SC_ABAJO] = KEY_DOWNARROW,
    [BMO_SC_IZQUIERDA] = KEY_LEFTARROW,
    [BMO_SC_DERECHA] = KEY_RIGHTARROW,
    /* Navegacion: el menu de guardar las usa, y `Supr` borra. */
    [BMO_SC_INSERT] = KEY_INS, [BMO_SC_SUPR] = KEY_DEL,
    [BMO_SC_INICIO] = KEY_HOME, [BMO_SC_FIN] = KEY_END,
    [BMO_SC_REPAG] = KEY_PGUP, [BMO_SC_AVPAG] = KEY_PGDN,
    /* El numerico, que es con lo que se jugaba de verdad en un 486. */
    [BMO_SC_KP_1] = KEYP_1, [BMO_SC_KP_2] = KEYP_2, [BMO_SC_KP_3] = KEYP_3,
    [BMO_SC_KP_4] = KEYP_4, [BMO_SC_KP_5] = KEYP_5, [BMO_SC_KP_6] = KEYP_6,
    [BMO_SC_KP_7] = KEYP_7, [BMO_SC_KP_8] = KEYP_8, [BMO_SC_KP_9] = KEYP_9,
    [BMO_SC_KP_MENOS] = KEYP_MINUS, [BMO_SC_KP_MAS] = KEYP_PLUS,
    [BMO_SC_KP_POR] = KEYP_MULTIPLY, [BMO_SC_KP_ENTRE] = KEYP_DIVIDE,
};

/* -- La SEGUNDA tecla de una misma tecla -------------------------------
 *
 * ** Aqui esta lo que hacia falta para no tener que elegir.
 *
 * La tabla vieja mandaba `KEY_UPARROW` cuando se pulsaba la `W`, y con eso el
 * juego andaba... pero la `W` dejaba de ser una letra: no habia forma de
 * escribir el nombre de una partida ni de teclear un truco. Y al reves, poner
 * `W` como letra deja el WASD sin movimiento.
 *
 * No hay que elegir, porque **un scancode puede producir DOS teclas de DOOM**:
 * `I_GetEvent` llama a `DG_GetKey` en bucle hasta que dice que no hay mas, asi
 * que la segunda se entrega en la vuelta siguiente del MISMO fotograma.
 *
 * Pulsar `A` manda entonces `'a'` (para el truco) y `KEY_STRAFE_L` (para el
 * paso lateral), y soltarla manda las dos sueltas. Lo de "las dos", literal:
 * si solo se emitiera una de las dos caras, DOOM se quedaria con la tecla
 * pegada para siempre.
 *
 * El menu no se confunde: en el modo de escribir un nombre, `M_Responder`
 * pasa la tecla por `toupper` y descarta lo que no cae en su fuente --
 * `KEY_STRAFE_L` vale 0xa0 y no cae--, asi que la letra entra sola. */
static unsigned char g_extra[128] = {
    [BMO_SC_W] = KEY_UPARROW,
    [BMO_SC_S] = KEY_DOWNARROW,
    [BMO_SC_A] = KEY_STRAFE_L,
    [BMO_SC_D] = KEY_STRAFE_R,
};

static int g_pendiente;                 /* hay una segunda tecla guardada? */
static int g_pendiente_pulsada;
static unsigned char g_pendiente_tecla;

/* Devuelve 1 si habia evento. `pressed` y `doomKey` son de salida. */
int DG_GetKey(int *pressed, unsigned char *doomKey)
{
    unsigned long long e;
    int sc;
    int tecla;
    int pulsada;

    /* La segunda tecla de la anterior va antes que nada: si se leyera un
     * evento nuevo primero, las dos caras de una misma tecla podrian salir en
     * orden cambiado. */
    if (g_pendiente) {
        g_pendiente = 0;
        *pressed = g_pendiente_pulsada;
        *doomKey = g_pendiente_tecla;
        return 1;
    }
    if (g_entrada == 0 && g_sup == 0) {
        return 0;
    }
    /* Se descartan aqui dentro las teclas que DOOM no conoce, en un bucle y no
     * con un `return 0`: devolver "no hay evento" por una tecla que DOOM no usa
     * dejaria las siguientes en la cola hasta el fotograma que viene, y con
     * varias seguidas la entrada se retrasaria sola. */
    for (;;) {
        /* ** DOS FUENTES, UN FORMATO: una ranura del buzon es el mismo evento
         * crudo que da la entrada exclusiva, asi que de aqui para abajo no hay
         * que saber de donde vino. */
        if (g_sup != 0) {
            e = bmo_superficie_evento(g_sup);
        } else {
            e = bmo_entrada_evento(g_entrada);
        }
        if ((e & BMO_EVENTO_HAY) == 0) {
            return 0;
        }
        /* El bit 63 se pregunta ANTES de leer el byte bajo: en un evento de
         * raton ese byte son los BOTONES, y leido como scancode cada clic
         * seria la tecla 1. DOOM no usa el raton aqui: se descarta. */
        if (bmo_sup_es_raton(e) == 1) {
            continue;
        }
        g_med_crudas = g_med_crudas + 1;
        sc = (int)(e & 0xFF);
        /* La palanca de la escala, y se come el evento: sigue el bucle sin
         * entregar nada. Solo al PULSAR -- si se hiciera en las dos caras, una
         * pulsacion saltaria dos escalas. */
        if (sc == BMO_SC_BLOQ_DESPL) {
            /* En ventana la superficie tiene el tamano con el que nacio: no
             * hay escala que cambiar, y se dice en vez de callar. */
            if ((e & BMO_EVENTO_PULSADA) != 0) {
                if (g_sup != 0) {
                    printf("DOOM: en ventana la escala es fija (x%d)\n", g_escala);
                } else {
                    cambiar_escala();
                }
            }
            continue;
        }
        /* La consola: ceder la pantalla y volver. Se come igual que la de
         * arriba, y solo al PULSAR -- en las dos caras cederia y volveria en la
         * misma pulsacion, que se ve como que no hace nada. */
        if (sc == BMO_SC_F12) {
            /* En ventana no hay pantalla que ceder: la consola ya se ve al
             * lado. La tecla se sigue comiendo, que DOOM no la use por error. */
            if ((e & BMO_EVENTO_PULSADA) != 0 && g_sup == 0) {
                ceder_o_volver();
            }
            continue;
        }
        if (sc < 128) {
            tecla = (int)g_tabla[sc];
            if (tecla != 0) {
                pulsada = (e & BMO_EVENTO_PULSADA) != 0;
                if (g_extra[sc] != 0) {
                    g_pendiente = 1;
                    g_pendiente_pulsada = pulsada;
                    g_pendiente_tecla = g_extra[sc];
                }
                *pressed = pulsada;
                *doomKey = (unsigned char)tecla;
                g_med_teclas = g_med_teclas + 1;
                return 1;
            }
        }
    }
}

/* -- 2.5  DG_SetWindowTitle --------------------------------------------- */

/* No hay barra de titulo que escribir: DOOM tiene la pantalla entera. Se deja
 * salir por consola una vez, que es donde se ve el nombre del WAD cargado. */
void DG_SetWindowTitle(const char *title)
{
    printf("%s\n", title);
}

/* -- 2.9  EL AUDITOR DEL MONTON ----------------------------------------
 *
 * ** El `Z_CheckHeap` de DOOM MATA, y matar no es informar.
 *
 * Metal del 14-08: el monton ya sale roto **antes del primer fotograma**, o sea
 * que lo destroza el ARRANQUE de DOOM y no `P_SetupLevel` -- el nivel solo era
 * el primero en mirar. Y `doom640.bex`, que no lleva sonda, jugo 171 tics
 * enteros a 43 fps con el monton ya roto: **la corrupcion es LATENTE**, en un
 * bloque que nadie usa.
 *
 * ★★★ SEGUNDA CORRIDA DEL MISMO DIA, y estrecha el caso 111 veces: con los
 * ocho puntos de control puestos, **`R_Init` sale SANO** (1129 bloques) y
 * tambien P_Init, S_Init, D_CheckNetGame, HU_Init y ST_Init (1334). El unico
 * sitio que queda es `D_DoomLoop`, o sea **DOCE bloques**. Lo de arriba --"lo
 * destroza el arranque"-- era cierto en el sentido flojo (pasa antes del primer
 * fotograma) y FALSO en el que importaba (no es `R_Init`).
 *
 * ★★ Y el ancho del disparo acusa solo. La foto fue:
 *
 *     BLOQUE 1336 en +1889056: dice 0, hasta el siguiente hay 672
 *                              | tag 1  id 1d4a11  (id BUENO)
 *
 * `memblock_t` es size(0) relleno(4) user(8) tag(16) id(20) next(24) prev(32).
 * Con `tag` e `id` INTACTOS y `size` a cero, lo escrito son **exactamente los 8
 * primeros bytes de la cabecera**. Eso descarta el desbordamiento: un `memset`
 * o un `rep stosb` pasado de largo se lleva `tag` e `id` por delante y esta
 * linea diria `id PISADO`. Lo que encaja es **UN almacenamiento suelto del
 * ancho de un puntero, con valor 0**.
 *
 * ★ Y en `z_zone.c` hay UNA sola linea asi: `Z_Free` -> `*block->user = 0`.
 * `user` es una direccion que da el llamante (`&lumpcache[lump]`, `&ptr`), o
 * sea **el mismo genero de fallo que ya se cazo tres veces en `Expr::AddrOf` y
 * en `pointer_scale`**. Sospechoso numero uno; no probado.
 *
 * Con eso, `Z_CheckHeap` deja de servir: dice "roto" y se lleva el programa por
 * delante sin decir donde ni de que. Esto camina la misma lista y **cuenta lo
 * que ve**.
 *
 * ★★ Y lo que decide el caso es UN campo: `id`. Cada bloque lleva `ZONEID`
 * (0x1d4a11) grabado en su cabecera.
 *
 *     id CORRECTO y tamano mal  -> la cabecera esta intacta, luego el que se
 *                                  equivoca es el ASIGNADOR (aritmetica), no
 *                                  un vecino que escribio de mas.
 *     id BASURA                 -> alguien PISO la cabecera: es un desbordamiento
 *                                  del bloque de antes, y `tag` dice de que clase.
 *
 * Son dos bugs completamente distintos y se distinguen mirando un entero.
 *
 * [!] Se puede llegar a `mainzone` porque es un `static` de `z_zone.c` y esto es
 * un unity build: un `static` de fichero es visible desde aqui hacia abajo. No
 * se toca una linea de DOOM para leerlo.
 *
 * [!] El tope de vueltas no es paranoia: si un `next` esta podrido, la lista
 * deja de tener final y un auditor sin tope no vuelve nunca. */
#define ZONEID_BMO 0x1d4a11
#define TOPE_BLOQUES 400000

/* -- LA TABLILLA, y hay que leer por que es una tablilla y no un arreglo ---
 *
 * ** El destrozo es LATENTE: DOOM jugo 129 tics con el dentro y solo se murio
 * cuando `Z_CheckHeap` miro, al final de `G_DoLoadLevel`. O sea que lo unico
 * que impide jugar --y por tanto MEDIR: escalas, teclado, fps-- es la mirada,
 * no el dano.
 *
 * Y el dano es UN campo. La lista de bloques se recorre por `next`, que esta
 * bien; el invariante que `Z_CheckHeap` exige es `b + b->size == b->next`, y
 * `b->next` es la respuesta correcta escrita al lado. Reponerlo es aritmetica,
 * no adivinanza.
 *
 * [!] Solo se remienda si `id` es el bueno. Con la cabecera PISADA no se sabe
 * si `next` es de fiar, y una tablilla sobre un hueso que no es el suyo hace
 * mas dano que la cojera.
 *
 * ⚠ LO QUE NO ARREGLA: quien escribio ese cero sigue suelto. Por eso el
 * remiendo se CUENTA y se dice en voz alta en cada linea, y por eso la sonda de
 * `jugando` sigue puesta -- si el numero de remiendos CRECE mientras se juega,
 * hay un segundo destrozo y eso hay que saberlo antes de arreglar el primero.
 * Se pone a 0 el dia que el culpable tenga nombre. */
#define TABLILLA_DEL_TAMANO 1

static int g_dueno_malo;

/* -- ** EL AUDITOR NO SE REPITE, y esto no es cosmetica (2026-08-31) --------
 *
 * `jugando` audita CADA fotograma y hasta hoy escribia sus tres o cuatro
 * lineas cada vez. A 28 fps eso son ~110 lineas por segundo, todas iguales.
 *
 * *** Y el precio no es el ruido: es que **la consola tiene fondo**. En las
 * cinco corridas de estos dos dias el dueno perdio el principio del log
 * SIEMPRE, y el principio es donde esta el cepo -- la unica linea que dice QUE
 * LLAMADA rompio el bloque. La respuesta que se dio entonces fue reordenar el
 * resumen para que el veredicto fuera primero: eso ataca el sintoma. Esto
 * ataca la causa.
 *
 * > Un instrumento que repite lo mismo 110 veces por segundo no informa mas:
 * > **borra lo que informaba**.
 *
 * Se compara la foto --el sitio, el primer roto, cuantos, y cuantos bloques--
 * con la de la vuelta anterior. Si es identica no se dice nada y se cuenta;
 * cuando algo CAMBIA se dice cuantas se callaron. El remiendo se sigue haciendo
 * igual: lo que se calla es la voz, no el trabajo.
 *
 * [!] Habla DOS veces al estrenar una foto y no una: `hablar` se decide antes
 * de recorrer la lista, o sea con lo que se sabia en la vuelta anterior. Dos es
 * barato, y de paso confirma que la foto es estable en vez de un parpadeo. */
static char *g_ult_donde;
static int g_ult_off;
static int g_ult_malos;
static int g_ult_vueltas;
static int g_callados;
static int g_estreno;
/* ** SANO NO HABLA (2026-09-21). El dueno lo pidio con el `save` en la mano:
 * 482 renglones de `paso N: Z_Malloc` y `jugando: SANO` cada pocos fotogramas
 * taparon la sesion entera y el lo leyo como un bucle infinito. Desde que DOOM
 * se juega y su plan esta cerrado, una auditoria que no encuentra nada es un
 * dato que ya se tiene. Se cuentan (las dice `bmo_vigilar(0)` en un renglon) y
 * solo habla la que encuentra algo. Poner esto a 1 devuelve el comportamiento
 * de antes, para cuando haga falta VER el orden de los pasos. */
static int g_hablar_sano = 0;
static int g_sanas = 0;

static int auditar_monton(char *donde)
{
    memblock_t *b;
    memblock_t *fin;
    unsigned long long base;
    int vueltas;
    int malos;
    int libres;
    int remendados;
    /* -- ** EL PRIMER ROTO, PARA QUE VIAJE EN EL RESUMEN (2026-08-30) ------
     *
     * La autopsia de Ring 3 solo guarda LA ULTIMA linea que escribio el
     * programa, y el renglon `ultimo` deja **62 caracteres**. O sea que el
     * detalle que se imprime arriba --el bloque, el tag, si el id esta pisado--
     * NO llega: lo tapa el resumen que va detras.
     *
     * *** Y el detalle es todo el diagnostico. `id BUENO` manda al ASIGNADOR;
     * `id PISADO` manda a buscar QUIEN escribio encima. Son dos investigaciones
     * distintas, y la linea que sobrevive no decia cual.
     *
     * Asi que el resumen lleva ahora lo del PRIMER roto, y lo lleva DELANTE:
     * lo que se corta es la cola, asi que el veredicto va donde no se corta. */
    int primer_off;
    int primer_tag;
    int primer_pisado;
    /* ** El `size` CRUDO del primer roto. Sin el, el veredicto no puede
     * distinguir un cero escrito encima de un numero mal calculado. */
    int primer_size;
    char *veredicto;
    int hablar;
    int mismo;
    unsigned int crudo;
    unsigned int tras;
    int relee;
    int off_ant;
    int tam_ant;
    int tag_ant;
    int off_ant2;
    int tam_ant2;
    int tag_ant2;

    if (mainzone == 0) {
        printf("[heap] %s: no hay zona todavia\n", donde);
        return 0;
    }
    base = (unsigned long long)mainzone;
    fin = &mainzone->blocklist;
    vueltas = 0;
    malos = 0;
    libres = 0;
    remendados = 0;
    primer_off = -1;
    primer_tag = 0;
    primer_pisado = 0;
    primer_size = 0;
    /* La foto anterior decide si esta habla. Ver el bloque de arriba. */
    hablar = 1;
    if (g_estreno != 0 && donde == g_ult_donde && g_callados > 0) {
        hablar = 0;
    }
    off_ant = -1;
    tam_ant = 0;
    tag_ant = 0;
    off_ant2 = -1;
    tam_ant2 = 0;
    tag_ant2 = 0;

    for (b = mainzone->blocklist.next; ; b = b->next) {
        if (b == fin || b->next == fin) {
            break;
        }
        vueltas = vueltas + 1;
        if (vueltas > TOPE_BLOQUES) {
            printf("[heap] %s: la lista NO TERMINA (mas de %d bloques)\n",
                   donde, TOPE_BLOQUES);
            return -1;
        }
        /* [!] Aqui ponia `b->tag == 0`, y por eso TODAS las lineas del metal
         * dijeron `(0 libres)`: **`PU_FREE` es 4, no 0** (`z_zone.h`: el enum
         * arranca en `PU_STATIC = 1`). La columna no estaba diciendo "no hay
         * huecos", estaba diciendo nada. No afecta al veredicto --el ROTO sale
         * del invariante de tamanos-- pero un cero que siempre es cero acaba
         * leyendose como un dato. */
        if (b->tag == PU_FREE) {
            libres = libres + 1;
        }
        /* El mismo criterio exacto que `Z_CheckHeap`, pero contando. */
        if ((unsigned long long)b + (unsigned long long)b->size
                != (unsigned long long)b->next) {
            malos = malos + 1;
            if (primer_off < 0) {
                primer_off = (int)((unsigned long long)b - base);
                primer_tag = b->tag;
                primer_pisado = (b->id != ZONEID_BMO);
                primer_size = b->size;
            }
            if (hablar && malos <= 3) {
                printf("[heap] %s: BLOQUE %d en +%d: dice %d, hasta el siguiente hay %d",
                       donde, vueltas,
                       (int)((unsigned long long)b - base),
                       b->size,
                       (int)((unsigned long long)b->next - (unsigned long long)b));
                /* [!] Aqui habia un ternario devolviendo literales de cadena, y
                 * se quito: **el censo de C no tiene ni una casilla para esa
                 * forma**. Un instrumento no puede apoyarse en algo sin medir
                 * -- si mintiera, mentiria justo en la linea que decide el
                 * diagnostico. Dos `if` cuestan cuatro lineas y no se discuten. */
                printf("  | tag %d  id %x", b->tag, b->id);
                /* ** ESTO AFIRMABA LO QUE NO SABE, y este mismo fichero
                 * lo tiene demostrado veinte lineas mas abajo: *"el
                 * veredicto ASIGNADOR era una heuristica y era FALSA ...
                 * el `id` esta intacto porque el que escribe solo llega a
                 * los DOS PRIMEROS BYTES"*.
                 *
                 * `id` vive en el offset 20. Un escritor que solo toca los
                 * primeros bytes lo deja entero, y decir "falla el
                 * ASIGNADOR" mandaba la investigacion al sitio contrario.
                 *
                 * Ahora hay TRES estados, y el tercero es una respuesta. */
                if (b->id != ZONEID_BMO) {
                    printf("  (id PISADO: alguien escribio hasta el +20)\n");
                } else if (b->size == 0) {
                    printf("  (size CERO con id bueno: pisado por delante)\n");
                } else {
                    printf("  (id bueno y size raro: NO SE SABE)\n");
                }

                /* -- ★ LA LECTURA CRUDA, y por que es la pregunta que falta -
                 *
                 * ** El 31-08 la sonda del dueno contesto `0 dueno`: la unica
                 * linea de DOOM que escribe un cero del ancho de un puntero
                 * NO disparo ni una vez. Luego nadie escribio ese cero por
                 * ahi. Y sin embargo hay un dato que llevaba TRES corridas
                 * delante sin que nadie lo leyera:
                 *
                 * > la tablilla escribe 672 en `b->size`, cuenta `1 rem`,
                 * > y en la auditoria siguiente el campo vuelve a decir 0.
                 * > **El mismo numero, exacto, fotograma tras fotograma.**
                 *
                 * Un vandalo suelto no es tan puntual. Lo que si es asi de
                 * puntual es una LECTURA que miente siempre igual.
                 *
                 * *** Asi que se leen los mismos cuatro bytes por otro camino:
                 * sin nombre de campo, sin `->`, un puntero y una suma. Si el
                 * campo dice 0 y los bytes dicen 672, la memoria esta SANA y
                 * el que se equivoca es el codigo que genera `b->size`. Es la
                 * misma jugada que el `id`: un valor parte el caso en dos.
                 *
                 *     crudo == campo   -> la memoria dice 0 de verdad
                 *     crudo != campo   -> la memoria esta bien y MIENTE EL
                 *                         ACCESO. bmo-cc, no DOOM.
                 *
                 * [!] `volatile` no sirve aqui: bmo-cc lo reconoce y lo tira
                 * (`declarations.rs:866`). Da igual -- lo que se necesita no
                 * es prohibir una optimizacion, es leer por OTRA ruta. */
                crudo = *(unsigned int *)b;
                printf("[heap]   CRUDO: los 4 bytes en +%d valen %u,"
                       " y el campo dice %d\n",
                       (int)((unsigned long long)b - base), crudo, b->size);

                /* -- ★★ EL VECINO DE ARRIBA, y por que es LA pregunta ------
                 *
                 * Los dos cortes anteriores salieron FALSOS y los dos por lo
                 * mismo: buscaban al que escribe entre el codigo que sabe que
                 * existe una zona. Y los hechos que quedan no encajan con eso:
                 *
                 *     la memoria dice 0 DE VERDAD (crudo == campo)
                 *     el remiendo SI llega (la tablilla nunca se quejo)
                 *     `Z_Free` no dispara a nadie (0 dueno)
                 *     y vuelve a 0 EN CADA FOTOGRAMA, misma direccion
                 *
                 * ** Algo escribe ahi ~28 veces por segundo y no es el
                 * asignador. Lo que corre a esa frecuencia es UNA cosa:
                 * **pintar**. Y en DOOM la pantalla vive DENTRO de la zona --
                 * `I_VideoBuffer = Z_Malloc(320*200, PU_STATIC, NULL)`, o sea
                 * 64.000 bytes con tag 1, exactamente el tag del bloque roto.
                 *
                 * > Un rasterizador que se pasa UNA fila por abajo escribe
                 * > justo detras del buffer. Y detras del buffer hay una
                 * > cabecera de bloque, cuyo primer campo es `size`.
                 *
                 * *** Asi que se imprime QUIEN ESTA JUSTO ENCIMA. Si el vecino
                 * mide 64.040 (64.000 y la cabecera) el caso esta cerrado sin
                 * mas corridas: el que escribe no es el monton, es el DIBUJO, y
                 * hay que ir a buscar el recorte que falta.
                 *
                 * Y se dice ademas donde ACABA `I_VideoBuffer`, que es la
                 * comprobacion directa: si acaba en este mismo offset, no hay
                 * nada que interpretar. */
                printf("[heap]   VECINDARIO: -2 en +%d (%d bytes, tag %d)"
                       " | -1 en +%d (%d bytes, tag %d)\n",
                       off_ant2, tam_ant2, tag_ant2,
                       off_ant, tam_ant, tag_ant);
                if (I_VideoBuffer != 0) {
                    {
                    unsigned long ar; unsigned long mi; unsigned char *ca;
                    void bmo_canario_cuentas(unsigned long *a, unsigned long *m,
                                             unsigned char **c);
                    char *dd; int v0; int v1;
                    char *bmo_canario_veredicto(int *x, int *y);
                    bmo_canario_cuentas(&ar, &mi, &ca);
                    dd = bmo_canario_veredicto(&v0, &v1);
                    printf("[heap]   CANARIO: armado %lu veces, mirado %lu;"
                           " apunta a +%d y el roto esta en +%d\n",
                           ar, mi,
                           ca == 0 ? -1 : (int)((unsigned long long)ca - base),
                           (int)((unsigned long long)b - base));
                    {
                        unsigned long bmo_filas_fuera(void);
                        unsigned long ff = bmo_filas_fuera();
                        if (ff > 0) {
                            printf("[heap]   *** FILAS FUERA DE PANTALLA"
                                   " SALTADAS: %lu
", ff);
                        }
                    }
                    if (dd != 0) {
                        printf("[heap]   *** EL CANARIO CAZO EN: %s"
                               " -- vio los bytes %d %d\n", dd, v0, v1);
                    }
                }
                printf("[heap]   PANTALLA: I_VideoBuffer en +%d, %d bytes,"
                           " ACABA en +%d\n",
                           (int)((unsigned long long)I_VideoBuffer - base),
                           320 * 200,
                           (int)((unsigned long long)I_VideoBuffer - base)
                               + 320 * 200);
                }
            }
            /* La tablilla. Va FUERA del `if (malos <= 3)`: ese tope es del
             * papel, no del hueso -- se remienda todo lo remendable aunque
             * solo se impriman los tres primeros. */
            if (TABLILLA_DEL_TAMANO && b->id == ZONEID_BMO) {
                relee = (int)((unsigned long long)b->next
                              - (unsigned long long)b);
                b->size = relee;
                remendados = remendados + 1;
                /* ★ Y SE RELEE LO QUE SE ACABA DE ESCRIBIR.
                 *
                 * Esto cierra el caso sin esperar al fotograma siguiente. La
                 * tablilla lleva tres corridas escribiendo 672 y contandolo, y
                 * el campo vuelve a decir 0 cada vez. O alguien lo pisa entre
                 * medias --y ahora hay que ser MUY rapido para eso, son dos
                 * instrucciones-- o **el almacenamiento no llega a la
                 * memoria**. Aqui se ve en el acto y por los dos caminos. */
                tras = *(unsigned int *)b;
                if (b->size != relee || tras != (unsigned int)relee) {
                    printf("[heap]   TABLILLA: escribi %d; el campo dice %d,"
                           " los bytes dicen %u  <- EL STORE NO LLEGA\n",
                           relee, b->size, tras);
                }
            }
        }
        /* El de antes del de antes, para poder mirar dos casas hacia arriba. */
        off_ant2 = off_ant;
        tam_ant2 = tam_ant;
        tag_ant2 = tag_ant;
        off_ant = (int)((unsigned long long)b - base);
        tam_ant = b->size;
        tag_ant = b->tag;
    }
    /* ** La foto de esta vuelta contra la de la anterior. */
    mismo = (g_estreno != 0 && donde == g_ult_donde && primer_off == g_ult_off
             && malos == g_ult_malos && vueltas == g_ult_vueltas);
    if (mismo && hablar == 0) {
        g_callados = g_callados + 1;
        if (malos == 0) {
            g_sanas = g_sanas + 1;
        }
        return malos;
    }
    if (mismo) {
        g_callados = g_callados + 1;
    } else {
        /* El recuento de identicas solo importa si las sanas hablan: si no,
         * seria un renglon para decir que no se dijo nada. */
        if (g_callados > 0 && g_hablar_sano) {
            printf("[heap] (%d auditorias identicas, calladas)\n", g_callados);
        }
        g_callados = 0;
    }
    g_estreno = 1;
    g_ult_donde = donde;
    g_ult_off = primer_off;
    g_ult_malos = malos;
    g_ult_vueltas = vueltas;
    if (malos == 0) {
        g_sanas = g_sanas + 1;
        if (g_hablar_sano) {
            printf("[heap] %s: SANO, %d bloques (%d libres)\n", donde, vueltas, libres);
        }
    } else {
        /* ** El orden NO es de gusto: es por donde corta la autopsia. Lo que
         * decide la investigacion --PISADO o ASIGNADOR-- va primero; el
         * recuento, que es contexto, va detras y puede perderse sin dano. */
        /* ** TRES ESTADOS, Y NINGUNO ADIVINA. (2026-09-02)
         *
         * El binario de antes tenia una rama verdadera y otra falsa:
         *
         *    id roto      -> PISADO         cierto: alguien llego al +20
         *    id intacto   -> "ASIGNADOR"    FALSO: solo dice que no llego
         *
         * Y el caso de la fila 200 cayo justo en la mentira: `size` valia
         * 672 = 0x2A0, alguien puso a cero sus dos primeros bytes, y el
         * `id` --que vive en el +20-- siguio intacto. Tres corridas
         * leyendo "ASIGNADOR" mientras el culpable era un escritor.
         *
         * *** El tercer estado es el que faltaba, y `SIN-DECIDIR` no es
         * una respuesta pobre: es la unica honesta cuando los dos casos
         * siguen abiertos. Un instrumento que elige uno para no decir "no
         * se" no informa de menos -- informa MAL, y con autoridad.
         *
         *    PISADO       el id esta roto. Alguien escribio hasta el +20
         *    PISADO-BAJO  id bueno y size CERO: pisado solo por delante,
         *                 que es la firma exacta del caso de la fila 200
         *    SIN-DECIDIR  id bueno y size raro. Puede ser el asignador y
         *                 puede ser un escritor corto. NO SE SABE
         *
         * [!] Y sale el `size` crudo, que es el dato con el que quien lee
         * puede juzgar sin creerse la etiqueta. */
        if (primer_pisado) {
            veredicto = "PISADO";
        } else if (primer_size == 0) {
            veredicto = "PISADO-BAJO";
        } else {
            veredicto = "SIN-DECIDIR";
        }
        printf("[heap] %s: %s +%d t%d size %d  %d/%d rotos, %d rem, %d dueno\n",
               donde,
               veredicto,
               primer_off, primer_tag, primer_size,
               malos, vueltas, remendados, g_dueno_malo);
    }
    return malos;
}

/* El nombre corto que llama `d_main.c`. Se declara alli a mano porque este
 * fichero se compila DESPUES del unity. */
void bmo_auditar(char *donde)
{
    /* Se tira el numero a proposito: los dos ficheros que llaman a esto lo
     * declaran `void` a mano, y cambiar esa firma seria tocar tres sitios para
     * un dato que ahi no se usa. Quien lo necesita es el cepo, que llama al
     * auditor directamente. */
    (void)auditar_monton(donde);
}

/* -- 2b. LA SONDA DEL DUENO: coger la escritura ANTES de que ocurra ------
 *
 * ** El auditor de arriba encuentra el DANO. Esto busca la MANO.
 *
 * Lo que dijo el metal el 2026-08-31, jugando de verdad:
 *
 *     BLOQUE 1336 en +1889056: dice 0, hasta el siguiente hay 672
 *     tag 1  id 1d4a11   (id BUENO)
 *     ... y vuelve a salir ROTO EN CADA FOTOGRAMA, siempre el mismo
 *
 * Y en la misma corrida, la muerte:
 *
 *     #PF  PUNTERO NULO en 0+0x2c  ->  R_SortVisSprites+0x2c6
 *
 * *** Los dos son la MISMA escritura. `vissprite_t` es prev(0) next(8) x1(16)
 * x2(20) gx(24) gy(28) gz(32) gzt(36) startfrac(40) **scale(44 = 0x2c)**. Esa
 * pantalla es `ds->scale` con `ds` a NULO -- o sea, un `next` de la lista de
 * sprites que vale 0. Y `next` esta en el offset 8 de su estructura, igual que
 * el `size` roto esta en el offset 0 de la suya.
 *
 *     Dos victimas de distinta especie, un solo genero de herida:
 *     OCHO BYTES DE CERO EN UNA DIRECCION QUE NO TOCABA.
 *
 * ★ Y en las 56.465 lineas de DOOM hay UNA sola linea que escribe un cero del
 * ancho de un puntero en una direccion que le dio otro:
 *
 *     Z_Free:   *block->user = 0;
 *
 * `user` es `&lumpinfo[n].cache`, `&texturecomposite[t]`, `&ptr`... una
 * direccion CALCULADA por el llamante. Si sale mal, `Z_Free` la dispara.
 *
 * == Por que se puede comprobar sin adivinar nada ==
 *
 * Porque la zona tiene un invariante que se mantiene solo, en los tres unicos
 * sitios que tocan `user`:
 *
 *     Z_Malloc      base->user = user;  *base->user = result;
 *     Z_ChangeUser  block->user = user; *user = ptr;
 *     Z_Free        *block->user = 0;   block->user = NULL;
 *
 * ** Luego al entrar en `Z_Free`, `*block->user` TIENE que valer exactamente
 * `ptr`. Siempre. Sin excepcion y sin caso raro.
 *
 * > Si no vale `ptr`, ese puntero no apunta a donde cree que apunta,
 * > y el cero de la linea siguiente cae en casa ajena.
 *
 * No es una heuristica ni una sospecha: es una igualdad, y se comprueba en un
 * `if`. Lo que imprime no es "puede que", es "no".
 *
 * == YA NO CAMBIA LA CONDUCTA, y eso se retiro a proposito (2026-08-31) ==
 *
 * La primera version devolvia 0 cuando el invariante fallaba, y `Z_Free` se
 * saltaba la escritura. Era razonable **mientras la hipotesis estaba viva**:
 * contener el disparo era la mitad del experimento.
 *
 * El metal contesto `0 dueno` TRES corridas seguidas: el invariante no falla
 * nunca. O sea que la contencion no protege de nada -- y lo que queda es un
 * cambio de conducta dentro del asignador, escrito para una teoria muerta,
 * esperando a que dentro de tres semanas alguien lo lea y no sepa por que esta.
 *
 * > Un experimento terminado que deja puesto su aparato deja de ser un
 * > experimento y pasa a ser una rareza del codigo.
 *
 * Asi que la sonda **siempre deja escribir** y se queda solo con lo que valia:
 * la comprobacion. Es barata, es un invariante de verdad, y el dia que alguien
 * toque `user` en un cuarto sitio esto lo dira en voz alta.
 *
 * == Lo que contesto, para que no haya que repetirlo ==
 *
 *     no sale ni una linea            -> la hipotesis es FALSA, el cero lo
 *                                        escribe otro y hay que buscar fuera
 *                                        de `z_zone.c`
 *     salen lineas y el monton sana   -> es esta, y la linea dice de que
 *                                        bloque salio el puntero malo
 *     salen lineas y ademas DOOM deja
 *     de morirse en R_SortVisSprites  -> ademas es la MISMA, y el caso de la
 *                                        lista de sprites se cierra con el
 *
 * [!] Leer `*user` no anade riesgo: es la misma direccion que la linea de
 * abajo iba a ESCRIBIR. Lo unico que se filtra antes es el alineado y el
 * primer sitio de memoria, porque de un puntero podrido lo mas probable es
 * que sea un numero pequeno, y ahi conviene contarlo en vez de morirse. */
static int g_dueno_dicho = 0;
#define DUENOS_QUE_SE_CUENTAN 8

int bmo_dueno_sano(void **user, void *ptr, int tag, int tam)
{
    unsigned long long u;
    unsigned long long base;
    unsigned long long fin;
    void *dice;
    int donde;

    u = (unsigned long long)user;

    /* Un `void**` que no esta a 8 no lo calculo ningun `&` legitimo: es un
     * numero que se colo. Y por debajo de 4 KiB no hay nada de nadie. */
    if ((u & 7) != 0 || u < 0x1000) {
        g_dueno_malo = g_dueno_malo + 1;
        if (g_dueno_dicho < DUENOS_QUE_SE_CUENTAN) {
            g_dueno_dicho = g_dueno_dicho + 1;
            printf("[heap] DUENO IMPOSIBLE: user=%x (ni alineado ni mapeable)"
                   "  del bloque t%d de %d bytes\n",
                   (unsigned int)u, tag, tam);
        }
        /* [!] Aqui SI se para: escribir por un puntero que no esta ni
         * alineado ni en memoria mapeable no es un experimento, es un fallo
         * seguro. La contencion se quita donde era una teoria, no donde es
         * aritmetica. */
        return 0;
    }

    dice = *user;
    if (dice == ptr) {
        return 1;
    }

    /* Falla el invariante. Se dice DE DONDE sale el puntero: si cae dentro de
     * la zona se da el offset, que es el mismo numero con el que habla el
     * auditor y se compara de un vistazo con el +1889056 del bloque roto. */
    g_dueno_malo = g_dueno_malo + 1;
    if (g_dueno_dicho < DUENOS_QUE_SE_CUENTAN) {
        g_dueno_dicho = g_dueno_dicho + 1;
        base = (unsigned long long)mainzone;
        fin = base + (unsigned long long)mainzone->size;
        if (u >= base && u < fin) {
            donde = (int)(u - base);
            printf("[heap] DUENO MALO: user=+%d (DENTRO de la zona)"
                   "  dice %x, deberia %x  | bloque t%d de %d bytes\n",
                   donde, (unsigned int)(unsigned long long)dice,
                   (unsigned int)(unsigned long long)ptr, tag, tam);
        } else {
            printf("[heap] DUENO MALO: user=%x (fuera de la zona)"
                   "  dice %x, deberia %x  | bloque t%d de %d bytes\n",
                   (unsigned int)u, (unsigned int)(unsigned long long)dice,
                   (unsigned int)(unsigned long long)ptr, tag, tam);
        }
        printf("[heap] ^ se escribe igual: la sonda ya no contiene nada\n");
    }
    /* Se DEJA escribir. La hipotesis murio el 31-08 y el aparato del
     * experimento se retira con ella; la comprobacion se queda. */
    return 1;
}

/* -- 2c. EL CANARIO: quien pisa la fila 200 ---------------------------------
 *
 * ** El 2026-08-31 la sonda del vecindario cerro la mitad del caso, y lo cerro
 * con una resta que no admite interpretacion:
 *
 * ```text
 *    PANTALLA: I_VideoBuffer en +1825056, 64000 bytes, ACABA en +1889056
 *    BLOQUE 1336 en                                            +1889056
 * ```
 *
 * Sin un byte de margen. El vecino de arriba mide 64.040 --64.000 y su
 * cabecera-- o sea que **el bloque roto es la cabecera que va justo detras de
 * la pantalla**, y lo que la pisa escribe en la fila 200 de una imagen que
 * tiene 200 filas: de la 0 a la 199.
 *
 * *** El veredicto `ASIGNADOR` era una heuristica y era FALSA. Decia "el `id`
 * esta intacto, luego nadie escribio encima" -- y el `id` esta intacto porque
 * el que escribe solo llega a los DOS PRIMEROS BYTES. `size` vale 672 = 0x2A0;
 * poniendo a cero los dos primeros bytes ya da 0, y `tag` e `id` viven en los
 * offsets 16 y 20. La heuristica confundio "no llego hasta el id" con "no
 * escribio nadie".
 *
 * == Y por que no vale seguir leyendo codigo ==
 *
 * Porque `RANGECHECK` ESTA ENCENDIDO (`doomdef.h:42`) y sus guardias
 * --`R_DrawColumn`, `R_DrawSpan`, `R_DrawFuzzColumn`-- no han disparado ni una
 * vez. Los sospechosos evidentes ya contestaron que no.
 *
 * ** Asi que se deja de razonar y se BISECA EL FOTOGRAMA. Se pone el valor
 * bueno al empezar a pintar y se mira en cinco sitios; el primero que lo
 * encuentre pisado es el culpable, y no hace falta ninguna teoria para leerlo.
 *
 * [!] Un cepo con pestillo, como el otro: caza una vez y se calla. Un canario
 * que sigue cantando despues de cazar tapa su propia captura -- la leccion del
 * 30-08, aplicada sin tener que volver a aprenderla. */
static unsigned char *g_canario;
static int g_canario_cazo;
/* -- ** "NO CANTO" Y "NO SE EJECUTO" NO SON LO MISMO (2026-09-01) ----------
 *
 * ** Ocho puntos de control desplegados --comprobado en el disco-- y NINGUNO
 * canto, con la auditoria gritando en cada fotograma que el bloque esta roto.
 * Las dos cosas no pueden ser ciertas.
 *
 * *** Y llevo dos vueltas dando por hecho lo que no he medido: que el canario
 * SE EJECUTA. Un instrumento que no corre y un instrumento que corre y no ve
 * nada dan la misma salida --silencio-- y mandan a sitios opuestos.
 *
 * > Antes de creerse el silencio de un instrumento hay que saber si el
 * > instrumento respiro.
 *
 * Asi que se cuentan las dos cosas, y la auditoria las dice. Con eso:
 *
 *     armados=0            `D_Display` no pasa por aqui: el canario es codigo
 *                          muerto y las ocho lineas no significan nada
 *     armados>0 mirados=0  arma y nadie comprueba: fallan las llamadas
 *     los dos >0           corre de verdad, y entonces el que miente es la
 *                          DIRECCION: los dos instrumentos no miran el mismo
 *                          sitio, y por eso se imprime tambien `g_canario`
 */
static unsigned long g_canario_armados;
static unsigned long g_canario_mirados;
/* ** DONDE CAZO, GUARDADO -- y esto es la tercera vez que aprendo lo mismo.
 *
 * El canario cazo en el PRIMER fotograma, imprimio su linea, echo el cerrojo...
 * y esa linea se la comio el desplazamiento de la consola. Durante tres corridas
 * lei `armado 1, mirado 930` como "no pasa nada" cuando decia lo contrario.
 *
 * *** Es EXACTAMENTE el fallo del cepo, cometido otra vez: cerre el cepo para
 * que la captura fuera la ultima linea, y despues escribi un instrumento cuya
 * captura tambien es una linea suelta que se puede perder.
 *
 * > Una captura que solo se dice UNA VEZ no es una captura: es una loteria
 * > contra el tamano de la consola.
 *
 * Ahora el sitio se GUARDA y lo repite la auditoria en cada vuelta. */
static char *g_canario_donde;
static int g_canario_vio0;
static int g_canario_vio1;

void bmo_canario_armar(void)
{
    if (I_VideoBuffer == 0 || g_canario_cazo) {
        return;
    }
    /* La cabecera que va JUSTO detras de la pantalla. Se le repone su `size`
     * --672, o sea 0xA0 0x02-- que es exactamente lo que hace la tablilla. */
    g_canario = (unsigned char *)I_VideoBuffer + 320 * 200;
    g_canario[0] = 0xA0;
    g_canario[1] = 0x02;
    g_canario_armados = g_canario_armados + 1;
}

/* Las cuentas del canario, para que el auditor pueda contradecirle. */
void bmo_canario_cuentas(unsigned long *a, unsigned long *m, unsigned char **c)
{
    *a = g_canario_armados;
    *m = g_canario_mirados;
    *c = g_canario;
}

/* Y lo que CAZO, para que la auditoria lo repita en cada vuelta. `0` = todavia
 * no cazo nada. */
char *bmo_canario_veredicto(int *v0, int *v1)
{
    *v0 = g_canario_vio0;
    *v1 = g_canario_vio1;
    return g_canario_donde;
}

void bmo_canario(char *donde)
{
    g_canario_mirados = g_canario_mirados + 1;
    if (g_canario == 0 || g_canario_cazo) {
        return;
    }
    if (g_canario[0] == 0xA0 && g_canario[1] == 0x02) {
        return;
    }
    g_canario_donde = donde;
    g_canario_vio0 = (int)g_canario[0];
    g_canario_vio1 = (int)g_canario[1];
    printf("[heap] *** CANARIO: lo piso %s -- los dos bytes valen %d %d\n",
           donde, g_canario_vio0, g_canario_vio1);
    g_canario_cazo = 1;
}

/* -- 2d. LA FILA QUE NO EXISTE --------------------------------------------
 *
 * ** El canario cazo, y dijo donde: `R_RenderPlayerView`. Con eso la aritmetica
 * se cierra sola:
 *
 * ```text
 *    I_VideoBuffer  320x200, filas validas 0..199
 *    +64000         = fila 200, columna 0  -> la cabecera del bloque 1336
 *    los 2 bytes    = un span de DOS pixeles en esa fila
 * ```
 *
 * *** Y el culpable es un `>` donde todo el resto del fichero pone `>=`. En el
 * DOOM original, DOS guardias:
 *
 * ```text
 *    R_MapPlane   if (... || y > viewheight)        <- y == viewheight PASA
 *    R_DrawSpan   if (... || (unsigned)ds_y>SCREENHEIGHT)   <- 200 > 200 es falso
 * ```
 *
 * Los dos consistentes entre si, y por eso `RANGECHECK` --que ESTA encendido--
 * no disparo nunca. Sus vecinos del mismo `if` usan `>=`: `x2>=viewwidth`,
 * `ds_x2>=SCREENWIDTH`, `dc_yh>=SCREENHEIGHT`. Solo la fila usa `>`.
 *
 * > Un guardia que se equivoca en el borde no protege el 99% de los casos:
 * > protege todos menos el unico que pasa.
 *
 * == Por que se INFORMA y se SALTA, en vez de morir ==
 *
 * El original llama a `I_Error`, que mata el juego. Eso probaria el caso una vez
 * y dejaria al dueno sin DOOM. Saltarse la fila que no existe **no pierde nada**
 * --esa fila no se ve, esta fuera de la pantalla-- y para la corrupcion en el
 * sitio donde nace.
 *
 * [!] Se dice UNA VEZ por sitio y se cuenta el resto. La leccion del cepo y del
 * canario, aprendida a la tercera: una captura que se repite en cada fotograma
 * se come su propia prueba. */
static int g_fila_dicha_plane;
static int g_fila_dicha_span;
static unsigned long g_filas_fuera;

void bmo_fila_fuera(char *donde, int y, int x1, int x2)
{
    int dicha;
    g_filas_fuera = g_filas_fuera + 1;
    dicha = donde[2] == 'M' ? g_fila_dicha_plane : g_fila_dicha_span;
    if (dicha) {
        return;
    }
    if (donde[2] == 'M') {
        g_fila_dicha_plane = 1;
    } else {
        g_fila_dicha_span = 1;
    }
    printf("[fila] *** %s: fila %d FUERA (validas 0..%d), columnas %d..%d"
           " -- se salta\n", donde, y, 200 - 1, x1, x2);
}

/* Cuantas filas fuera van, para el resumen del monton. */
unsigned long bmo_filas_fuera(void)
{
    return g_filas_fuera;
}

/* -- EL CEPO: auditar DESPUES DE CADA LLAMADA A LA ZONA -----------------
 *
 * ** Metal del 14-08 (segunda corrida): los cuatro puntos de `D_DoomLoop`
 * dejaron la rotura dentro del **primer `doomgeneric_Tick()`**, y son TRES
 * bloques (1335 -> 1338). Con tres sospechosos ya no hace falta partir la
 * ventana: se mira una por una.
 *
 * `Z_Malloc` y `Z_Free` llaman aqui al terminar. Mientras el cepo esta
 * ARMADO --solo durante ese primer Tick-- cada llamada deja su linea, y **la
 * primera que diga ROTO es la culpable**, con el tamano que se pidio delante
 * para poder buscarla en el fuente.
 *
 * [!] Armado a mano y no siempre: cada paso camina los ~1338 bloques. Durante
 * un Tick eso es nada; durante una partida seria el juego.
 *
 * [!] El auditor NO reserva memoria, asi que llamarlo desde dentro del
 * asignador no puede reentrar. Es la razon de que se pueda hacer esto. */
static int g_cepo_armado = 0;
static int g_cepo_paso = 0;
/* -- ** EL PESTILLO: una vez cazo, NO SE VUELVE A ARMAR (2026-08-30) -----
 *
 * Hay DOS sitios que arman el cepo: `P_SetupLevel` (g_game.c) y el primer
 * `Tick` (d_main.c), en ese orden. Y el cepo se cierra solo al primer roto
 * para que **la linea culpable sea la ULTIMA**, que es la unica que la
 * autopsia de Ring 3 guarda.
 *
 * *** Sin este pestillo eso no serviria de nada: el segundo cepo se armaria
 * detras, imprimiria sus propios pasos, y la ultima linea volveria a ser una
 * cualquiera. Cazar y seguir hablando es no haber cazado. */
static int g_cepo_cazo = 0;

void bmo_vigilar(int encendido)
{
    if (encendido && g_cepo_cazo) {
        printf("[heap] cepo NO se rearma: ya cazo en el paso %d\n", g_cepo_paso);
        return;
    }
    g_cepo_armado = encendido;
    if (encendido) {
        g_cepo_paso = 0;
        printf("[heap] cepo ARMADO\n");
    } else {
        printf("[heap] cepo guardado tras %d pasos por la zona, %d auditorias sanas calladas\n",
               g_cepo_paso, g_sanas);
    }
}

void bmo_zona_paso(char *que, int tam)
{
    int rotos;

    if (g_cepo_armado == 0) {
        return;
    }
    g_cepo_paso = g_cepo_paso + 1;
    /* El paso solo se anuncia si las sanas hablan: cuando algo se rompe, el
     * CEPO CERRADO de abajo ya dice cual fue y de cuanto. */
    if (g_hablar_sano) {
        printf("[heap] paso %d: %s de %d bytes ->\n", g_cepo_paso, que, tam);
    }
    rotos = auditar_monton(que);

    /* -- ** EL CEPO SE CIERRA SOLO AL PRIMER ROTO (2026-08-30) ----------
     *
     * ** Antes seguia imprimiendo hasta que alguien lo desarmaba, y eso tiene
     * dos precios. El barato: miles de lineas por una que importa. El caro:
     *
     * > La autopsia de Ring 3 guarda **LA ULTIMA linea** que escribio el
     * > programa. Un cepo que sigue hablando despues de la rotura **tapa
     * > justo la linea que vino a buscar**.
     *
     * *** Cerrandolo aqui, la culpable ES la ultima linea. Es la misma
     * leccion del 30-08 --lo que decide va donde no se pierde-- aplicada al
     * unico sitio donde se puede aprovechar: el orden en que se imprime.
     *
     * [!] `rotos < 0` es "la lista no termina", que tambien para: seguir
     * caminando una lista circular rota es como se cuelga un diagnostico. */
    if (rotos != 0) {
        g_cepo_armado = 0;
        g_cepo_cazo = 1;
        printf("[heap] CEPO CERRADO: el paso %d (%s de %d) es el primero ROTO\n",
               g_cepo_paso, que, tam);
    }
}

/* -- 3.  EL METRO -------------------------------------------------------
 *
 * ** Esto existe porque "va lento" no es un dato y "se ve bien" tampoco.
 *
 * DOOM tiene la pantalla entera, asi que no hay donde pintar un contador sin
 * ensuciar el juego -- pero `printf` sigue yendo a la consola, y la consola
 * acaba en `datos/salida.txt` cuando el programa termina. O sea que el sitio
 * correcto para el instrumento es la salida de texto: se juega, se sale, y el
 * fichero cuenta lo que paso.
 *
 * Las cifras y lo que descarta cada una:
 *
 *   xN WxH       la escala con la que se midio esa linea. Va delante porque
 *                sin ella las demas no se pueden comparar entre si: el mismo
 *                juego a x2 y a x5 son dos maquinas distintas.
 *   MHz          ** a que va el NUCLEO, no el reloj de referencia. ~4600 es
 *                el boost de un solo nucleo de este Zen 3; ~3700 es la base.
 *                Si sale la base, alguien mas esta despierto.
 *   nucleos      cuantos estan en pie. **1 es la respuesta buena para un
 *                juego**: los otros once dormidos son lo que deja subir a
 *                este. Si dice 12, se ejecuto `smp` y los APs estan GIRANDO
 *                en `spin_loop()` -- ver la nota de `main`.
 *   fps          si baja de ~20, no se juega, y lo demas dice por donde.
 *   us/fotograma lo mismo del reves, que es lo que se compara con los 28.571
 *                us que dura un tic de DOOM (35 por segundo).
 *   us de blit   cuanto de eso es COPIAR a la pantalla. Si es pequeno, el
 *                lento es el renderizador y la copia directa al framebuffer
 *                no arreglaria nada.
 *   tics         si `gametic` no sube, el mundo no avanza aunque se pinte.
 *                Distingue "lento" de "colgado", que a ojo se parecen.
 *   teclas       eventos leidos del kernel / los que DOOM entiende. Si el
 *                primero es 0, el teclado no llega y no es cosa de DOOM; si
 *                el primero sube y el segundo no, es la tabla de arriba.
 */
/* Dos segundos y no cinco. **Las cinco corridas del 14-08 murieron a los ~4,8
 * s**, o sea justo antes de la primera linea periodica: del arranque entero
 * solo llego el resumen final. Un instrumento cuyo primer dato llega despues
 * de que el paciente se muera no es un instrumento. */
#define MEDIR_CADA_MS 2000

/* -- LA SONDA DEL MONTON -----------------------------------------------
 *
 * ** DOOM muere con `Z_CheckHeap: block size does not touch the next block`,
 * y esa llamada esta en UN SOLO SITIO: `g_game.c:658`, al final de
 * `G_DoLoadLevel`, despues de `P_SetupLevel`. O sea que **el monton solo se
 * mira una vez en toda la partida**, cuando ya se cargo el primer nivel.
 *
 * Y con una sola mirada no se puede saber nada: el destrozo pudo pasar ahi
 * mismo o media hora antes. La pregunta que hay que contestar primero no es
 * *quien* rompe el monton, sino **CUANDO** -- y son dos respuestas muy
 * distintas:
 *
 *   · si ya esta roto en el fotograma 1, el culpable es el ARRANQUE (`R_Init`
 *     y compania), y el nivel no tiene nada que ver: solo fue el primero en
 *     mirar.
 *   · si aguanta 170 tics sanos y revienta al entrar el demo, es
 *     `P_SetupLevel` -- el codigo que castea structs encima de los bytes
 *     crudos del WAD.
 *
 * Mirarlo en CADA fotograma convierte esa pregunta en una linea del fichero de
 * salida: la ultima `heap sano` que se imprima dice el ultimo instante bueno.
 *
 * [!] Es una SONDA, no una funcion del juego: recorre la lista de bloques
 * entera cada fotograma. Se quita en cuanto conteste. */
#define SONDA_DE_MONTON 1

static void medir(void)
{
    unsigned long long ahora;
    unsigned long long ms;
    int fps;
    int us;
    int us_blit;
    int us_exp;
    int us_vol;
    int mhz;
    int vivos;

    ahora = __rdtsc();
    if (g_med_t0 == 0) {
        g_med_t0 = ahora;
        g_med_arranque = ahora;
        return;
    }
    ms = (ahora - g_med_t0) / g_tsc_khz;
    if (ms < MEDIR_CADA_MS) {
        return;
    }

    /* Cero fotogramas en cinco segundos es una respuesta, no un error de
     * division: quiere decir que el bucle gira y no se pinta. */
    if (g_med_fotogramas > 0) {
        fps = (int)((unsigned long long)g_med_fotogramas * 1000 / ms);
        us = (int)(ms * 1000 / (unsigned long long)g_med_fotogramas);
        us_exp = (int)(g_med_exp * 1000 / g_tsc_khz
                       / (g_med_fotogramas == 0 ? 1 : g_med_fotogramas));
        us_vol = (int)(g_med_vol * 1000 / g_tsc_khz
                       / (g_med_fotogramas == 0 ? 1 : g_med_fotogramas));
        us_blit = (int)(g_med_blit * 1000 / g_tsc_khz
                        / (unsigned long long)g_med_fotogramas);
    } else {
        fps = 0;
        us = 0;
        us_blit = 0;
        us_exp = 0;
        us_vol = 0;
    }
    g_med_total = g_med_total + g_med_fotogramas;

    /* ** A QUE VELOCIDAD VA EL NUCLEO, que es la pregunta que ningun programa
     * de C podia hacer hasta hoy.
     *
     * `BMO_INFO_TSC_HZ` --el que usa el reloj de este fichero-- dice a que va
     * el RELOJ DE REFERENCIA, y ese no cambia nunca: 3,7 GHz encendida o a
     * medio gas. Esto otro dice a que va **el nucleo ahora**, que en un Zen 3
     * es 3,7 o 4,6 segun cuantos nucleos esten despiertos.
     *
     * Y esa diferencia es exactamente lo que separa "el sistema le dio un
     * nucleo a DOOM" de "el sistema le dio EL nucleo bueno a toda velocidad".
     * Con este numero al lado de los fps, la frase deja de ser una opinion.
     *
     * ★ Se pregunta UNA vez por linea a proposito: es una medida por
     * diferencia entre dos lecturas, asi que preguntarla dos veces seguidas
     * mide un intervalo de microsegundos y contesta cualquier cosa. */
    mhz = (int)(bmo_info(BMO_INFO_CPU_HZ_REAL) / 1000000);
    vivos = (int)bmo_info(BMO_INFO_SMP_VIVOS);

    printf("[perf] x%d %dx%d | %d fps | fotograma %d us, de ellos blit %d us | tics %d | teclas %d/%d | %d MHz, %d nucleos en pie\n",
           g_escala, g_dst_ancho, g_dst_alto,
           fps, us, us_blit, gametic - g_med_tic0, g_med_teclas, g_med_crudas,
           mhz, vivos);
    /* == LA VENTANA DE VISTA, EN LA LINEA QUE EL DUENO SI LEE ============
     *
     * ** El instrumento del 04-09 imprimia estos numeros en
     * `R_ExecuteSetViewSize`... que corre UNA vez, al arrancar, y para
     * entonces DOOM ya tiene la pantalla. O sea que la respuesta se escribia
     * en una consola que **nadie puede ver mientras el juego corre**.
     *
     * Es la misma familia de todo este mes: un instrumento que contesta donde
     * no hay quien lo lea no ha contestado. Los tres numeros viajan aqui,
     * pegados a la linea que ya se fotografia cada vez.
     *
     * `viewwidth` tiene que ser 320. Si sale otra cosa, alguien mueve la
     * ventana DESPUES de `R_Init` y ese alguien es el que falta por encontrar. */
    /* == 2026-09-09: ESTA LINEA MIRABA AL LADO EQUIVOCADO DE LA VALLA =====
     *
     * Imprimia `screenblocks` y `detailLevel`, que son las variables del MENU.
     * Y el que calcula `viewwidth` no las lee: lee `setblocks`, `setdetail` y
     * `detailshift`, que es donde `R_SetViewSize` deja sus argumentos.
     *
     * ** Por eso la foto decia `blocks=10 detalle=0` --los dos correctos-- con
     * un `viewwidth=80` que con esos numeros es IMPOSIBLE: 10*32 = 320, y
     * 320>>0 = 320. Los tres numeros eran ciertos y no hablaban del mismo
     * lado, que es la unica forma de que un instrumento mienta sin equivocarse.
     *
     * *** Es la misma familia de todo este mes: el pulso que media sin reloj,
     * el volcado que no decia el modo, el ritmo del bus que vivia en Ring 0.
     * Un instrumento tiene que leer la variable que DECIDE, no la que se le
     * parece. */
    printf("[perf]   vista: viewwidth=%d viewheight=%d anchoesc=%d"
           " (set %d/%d shift=%d | menu %d/%d)
",
           viewwidth, viewheight, scaledviewwidth,
           setblocks, setdetail, detailshift, screenblocks, detailLevel);
    /* == INSTRUMENTO (2026-09-04): LAS DOS MITADES ======================
     *
     * `c/blit.bex` dice que el framebuffer da 1,47 GB/s -> los 6,4 MB de un
     * fotograma deberian costar 4.360 us. Si `volcado` sale cerca de eso y
     * `expansion` se lleva el resto, el cuello NO es el bus: es el bucle que
     * expande la fila, y eso se arregla sin comprar nada. */
    printf("[perf]   mitades: expansion %d us  +  volcado %d us   (de %d)\n",
           us_exp, us_vol, us_blit);
    /* == INSTRUMENTO (2026-09-04): LOS SEGS QUE NO SE DIBUJAN ===========
     *
     * La pantalla partida --media vista de juego y media pantalla de titulo
     * debajo-- no es un fallo del volcado: es que a esas columnas **nadie las
     * pinto**. Un seg que entra con `x1 > x2` se descarta en `R_AddLine` sin
     * decir nada, y lo que queda en esa franja es el fotograma anterior.
     *
     * ** `fuera` tiene que ser CERO. Es un indice pasado del final de
     * `viewangletox[4096]`, o sea una lectura de basura, y con la aritmetica
     * de 32 bits correcta no puede ocurrir ni una vez.
     *
     * Si `fuera` sale 0 y `reves` tambien, la pantalla partida NO es del
     * renderizador y hay que ir al compositor -- que es justo lo que esta
     * linea evita adivinar. */
    printf("[perf]   segs: al reves %d | indice fuera de tabla %d   (cero y cero)\n",
           bmo_addline_reves, bmo_addline_fuera);
    bmo_addline_reves = 0;
    bmo_addline_fuera = 0;

    g_med_t0 = ahora;
    g_med_fotogramas = 0;
    g_med_blit = 0;
    g_med_exp = 0;
    g_med_vol = 0;
    g_med_tic0 = gametic;
}

/* El resumen, al salir. Se registra con `I_AtExit` --el mismo camino por el que
 * DOOM guarda su configuracion-- para que salga tambien cuando se sale por
 * F10, que es como se sale de verdad. */
static void informe_final(void)
{
    unsigned long long ms;

    ms = (__rdtsc() - g_med_arranque) / g_tsc_khz;
    g_med_total = g_med_total + g_med_fotogramas;
    printf("[perf] TOTAL: %d fotogramas en %d ms", g_med_total, (int)ms);
    if (ms > 0) {
        printf(" = %d fps de media", (int)((unsigned long long)g_med_total * 1000 / ms));
    }
    printf(" | %d tics | %d teclas de %d eventos | %d MHz, %d nucleos en pie\n",
           gametic, g_med_teclas, g_med_crudas,
           (int)(bmo_info(BMO_INFO_CPU_HZ_REAL) / 1000000),
           (int)bmo_info(BMO_INFO_SMP_VIVOS));
    /* ** Y EL BLIT EN EL RESUMEN, que en la primera tanda falto y era la cifra
     * que decidia el siguiente paso: con 24 fps a x5 y sin este numero no se
     * puede saber si el lento es el escalador o el renderizador. Un resumen
     * que no lleva lo mismo que la linea periodica es un resumen que obliga a
     * volver a arrancar la maquina. */
    if (g_med_total > 0) {
        printf("[perf] TOTAL: escala x%d %dx%d | blit %d us por fotograma\n",
               g_escala, g_dst_ancho, g_dst_alto,
               (int)(g_med_blit_total * 1000 / g_tsc_khz
                     / (unsigned long long)g_med_total));
    }
}

/* -- El arranque -------------------------------------------------------- */

/* ** EL WAD SE NOMBRA AQUI, y no se busca.
 *
 * `d_iwad.c` sabe rebuscar en directorios estandar y en variables de entorno.
 * Aqui no hay ni lo uno ni lo otro --`getenv` contesta que no hay, y lo dice en
 * `<stdlib.h>`-- asi que se le pasa la ruta por argumento, que es el camino que
 * DOOM ya tiene y el unico que no depende de inventarse un sistema de ficheros
 * que BMO-X no promete.
 *
 * `myargv` tiene que sobrevivir a `main`: `m_argv.c` guarda el puntero y lo
 * consulta durante toda la partida. Por eso es global y no local.
 *
 * ** FREEDOOM PRIMERO, Y SI NO ESTA, DOOM (2026-09-25, L0 de PLAN_LA_LUDOTECA).
 *
 * `apps/freedm1.wad` es Freedoom Fase 1: los cuatro episodios, libre (BSD), y
 * el build lo deja con ese nombre 8.3 si esta en `BMO-externo\doom\`. Si falta,
 * se abre `doom1.wad` como siempre. El orden importa: "doom1 si esta" no
 * usaria nunca Freedoom, porque `doom1.wad` esta siempre.
 *
 * Preguntar cuesta una apertura sin lectura: en FAT32 `obj::file::open` no
 * trae ni un byte, solo reserva el cursor (y `fclose` lo suelta). */
#define BMO_WAD_LIBRE "apps/freedm1.wad"
#define BMO_WAD_DOOM  "apps/doom1.wad"
static char *g_argv[3] = { "doom.bex", "-iwad", BMO_WAD_DOOM };

static void elegir_wad(void)
{
    if (M_FileExists(BMO_WAD_LIBRE)) {
        g_argv[2] = BMO_WAD_LIBRE;
    } else {
        g_argv[2] = BMO_WAD_DOOM;
    }
    printf("[bmo] wad: %s\n", g_argv[2]);
}

/* == LO QUE NO SE VE, NO SE PINTA (2026-09-11, R-APP8 de META-APP_HARD) ====
 *
 * ** DOOM YA SABIA HACER ESTO. Chocolate Doom tiene `screenvisible`, y
 * `doomgeneric_Tick` dice literalmente:
 *
 *     TryRunTics();                 la logica: sigue
 *     S_UpdateSounds(...);          el sonido: sigue
 *     if (screenvisible) D_Display();   el dibujo: SOLO si se ve
 *
 * Lo unico que faltaba era que alguien le dijera la verdad, y aqui la dice el
 * DIRECTOR: el byte 2 del buzon de la ventana, que se lee con
 * `bmo_superficie_se_ve`. Con la ventana minimizada, fuera de la pantalla o
 * con la pantalla prestada a otro, DOOM sigue jugando -- los monstruos se
 * mueven, la musica suena -- y deja de dibujar 960x600 para nadie.
 *
 * ** Se pregunta en el bucle de `main`, una vez por vuelta, y no en
 * `DG_DrawFrame`: ese solo se llama si se dibuja, asi que con la ventana
 * oculta nadie volveria a preguntar. A pantalla exclusiva (`g_sup == 0`) no se
 * toca: ahi siempre se ve. */
static void vista_al_dia(void)
{
    if (g_sup == 0) {
        return;
    }
    if (bmo_superficie_se_ve(g_sup)) {
        screenvisible = true;
    } else {
        screenvisible = false;
    }
}

/* ** EL BUCLE DEL JUEGO VA AQUI, Y ESTE FICHERO NO LO TENIA.
 *
 * Lo que habia escrito era: *"`doomgeneric_Create` llama a `D_DoomMain`, que no
 * vuelve: dentro esta el bucle del juego"*. **Es falso, y era el fallo.**
 *
 * `D_DoomLoop` se llama "loop" y NO ES UN BUCLE. Su ultima linea es una sola
 * llamada a `doomgeneric_Tick()` y vuelve. En `doomgeneric` el bucle lo pone la
 * PLATAFORMA, siempre -- se puede comprobar en los otros ocho backends del
 * mismo directorio (`_sdl`, `_soso`, `_xlib`, ...): los ocho tienen
 * `while (1) doomgeneric_Tick();` en su `main`, y ese es todo el trato.
 *
 * Sin el, DOOM hacia el arranque ENTERO --el WAD, las texturas, los sprites, la
 * pantalla-- pintaba **un** fotograma y salia por su propio pie. Y eso se ve
 * exactamente como un juego que arranca bien: la portada aparece, porque la
 * portada ES el primer fotograma. No se colgaba ni fallaba nada; se acababa.
 *
 * [!] La leccion, que es la de siempre en este arbol: el comentario afirmaba
 * algo del codigo AJENO --que una funcion no vuelve-- sin haberlo leido. Y una
 * afirmacion asi, escrita con seguridad, tapa el sitio del fallo mejor que el
 * silencio.
 *
 * Un `for` sin salida esta bien aqui: DOOM sale por dentro (`I_Quit` -> `exit`,
 * que es lo que hace F10), y el rescate `Ctrl+Alt+Esc` del kernel vigila la
 * puerta CRUDA de la entrada, que es justo la que usa este programa.
 *
 * == ** Y ESTE BUCLE TIENE UN NUCLEO ENTERO PARA EL, POR CONSTRUCCION ==
 *
 * No hay que pedirlo ni configurarlo, y conviene saber por que: en BMO-X el
 * planificador es **uno solo y vive en el BSP** (`scheduler::on_timer` lo llama
 * el timer del LAPIC del BSP, y de ningun otro). Los otros once nucleos no
 * tocan el planificador jamas: cuando se levantan, entran en `crew::obrero`,
 * que es un bucle de faenas repartidas, no una cola de tareas.
 *
 * O sea que una tarea de Ring 3 **no migra nunca**. Su L1 y su L2 no se
 * enfrian, y no hay un segundo planificador que pueda opinar. Eso es justo lo
 * que un juego de un solo hilo quiere y lo que un sistema de proposito general
 * no puede prometer.
 *
 * [!] ** Y AQUI HABIA UNA ADVERTENCIA QUE EL METAL TUMBO, que se deja escrita
 * porque el error importa mas que el acierto.
 *
 * Decia: *"si se ejecuta `smp` antes de jugar, los once APs esperan girando
 * (`spin_loop()`, o sea `pause` en C0), y en un Zen 3 el boost de un nucleo
 * depende de que los demas duerman -- caeria de ~4600 a ~3700 MHz"*.
 *
 * **Se midio y es FALSO.** Metal del 14-08, mismo binario, misma sesion:
 *
 *     1 nucleo   en pie  ->  4506 MHz
 *     12 nucleos en pie  ->  4501 MHz      <- CINCO megahercios
 *
 * Un bucle de `pause` no hace trabajo, luego no consume potencia de verdad, y
 * el presupuesto del paquete ni se entera. El razonamiento era plausible y la
 * medida vale mas: **`smp` no le quita el boost a nadie**. */
int main()
{
    int vigilado;

    printf("DOOM en BMO-X\n");
    I_AtExit(informe_final, 1);
    elegir_wad();
    doomgeneric_Create(3, g_argv);

    /* ** LA SONDA DE ANTES MATABA, Y YA CONTESTO LO SUYO.
     *
     * Preguntaba *"esta roto antes del primer fotograma?"* y el metal del 14-08
     * dijo que SI, o sea que el culpable es el arranque de DOOM. A partir de
     * ahi `Z_CheckHeap` estorbaba: se llevaba el programa por delante antes de
     * pintar nada, y con eso no se puede ni jugar ni seguir midiendo.
     *
     * Lo que hay ahora son los puntos de control repartidos por `D_DoomMain`
     * (ver `bmo_auditar`), que dicen **de que paso** y **de que bloque** sin
     * matar a nadie. Aqui solo queda una foto por si algo cambia jugando. */
    auditar_monton("antes del primer fotograma");

    vigilado = 0;
    for (;;) {
        /* R-APP8: antes de cada vuelta, la verdad sobre si se nos ve. */
        vista_al_dia();
        doomgeneric_Tick();

        /* == EL LATIDO DE DOOM (2026-09-09) ==========================
         *
         * El 09-09 DOOM dejo de imprimir a los ~25 s sin fallo y sin mensaje.
         * Y con la pantalla quieta hay DOS estados que se ven identicos:
         *
         *    se colgo dentro de un tick
         *    sigue vivo y lo que se callo fue el `[perf]`
         *
         * ** Los dos se ven igual, y se arreglan en sitios distintos. Una
         * linea cada 256 fotogramas --una cada cuatro segundos a 68 fps-- los
         * separa, y no molesta a nadie.
         *
         * *** Es la leccion de toda esta semana escrita una vez mas: el `pulso`
         * de la barra existe por lo mismo, y el `SIN RELOJ` tambien. Un
         * instrumento callado y un sistema muerto se ven igual. */
        g_latido = g_latido + 1;
        if ((g_latido & 255) == 0) {
            printf("[vivo] fotograma %d\n", g_latido);
        }

        /* -- ★★ EL PUNTO QUE FALTABA, y faltaba por MI culpa (2026-08-31) ----
         *
         * ** El canario se desplego y NO CANTO NI UNA VEZ, con la auditoria
         * gritando en cada fotograma que el bloque estaba roto. Las dos cosas
         * no pueden ser ciertas... salvo que se estorben, y se estorban:
         *
         * ```text
         *    doomgeneric_Tick()      D_Display arma y comprueba -> LIMPIO
         *                            ...y el blit del puerto, sin comprobar
         *    auditar_monton()        encuentra 0 y REMIENDA a 672
         *    vuelta siguiente        el canario ve 672 -> intacto -> calla
         * ```
         *
         * *** **La auditoria curaba la prueba antes de que el canario mirara.**
         * Dos instrumentos correctos, puestos en el orden equivocado, dando
         * entre los dos un resultado que parecia decir "no pasa nada".
         *
         * > Un instrumento que se cruza con otro no mide de menos: mide MAL, y
         * > lo hace en voz de dato.
         *
         * Aqui, antes de la auditoria, es el unico sitio donde la prueba sigue
         * cruda. Si canta AQUI y no en los siete de dentro, el que escribe esta
         * entre el final de `D_Display` y el retorno del Tick -- o sea en el
         * blit del puerto. */
        { void bmo_canario(char *d); bmo_canario("tras el Tick, ANTES de auditar"); }
        if (SONDA_DE_MONTON) {
            vigilado = vigilado + 1;
            /* ** VEINTE Y NO 175, y el numero lo eligio el metal. Con 175 la
             * corrida entera duro 120 fotogramas: **la sonda de jugando no
             * llego a dispararse NI UNA VEZ**, y justo en ese hueco ciego
             * aparecio el segundo destrozo (remendado en el fotograma 1, roto
             * otra vez para el tic 171). Un instrumento cuyo periodo es mas
             * largo que la vida del paciente no mide nada. */
            if (vigilado >= 20) {
                vigilado = 0;
                auditar_monton("jugando");
            }
        }
        medir();
    }
    return 0;
}
