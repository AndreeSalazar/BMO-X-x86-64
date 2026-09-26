/* bmo_opl.c -- UN SINTETIZADOR FM DE DOS OPERADORES, el del YM3812 (OPL2).
 *
 * La musica de DOOM no es sonido: son partituras (MUS) y un banco de
 * instrumentos (el lump GENMIDI) pensados para el chip FM de la AdLib y la
 * Sound Blaster. Este fichero es ese chip, escrito para BMO-X (2026-09-22):
 * se le escriben REGISTROS, como al de verdad, y devuelve muestras.
 *
 * == Escrito desde la hoja del chip, no copiado ==
 *
 * Lo que sale del YM3812 son HECHOS publicados: el mapa de registros, que un
 * canal son dos operadores, las cuatro ondas, las tablas de tiempos de ataque
 * y caida, la KSL. El codigo es propio. La precision tambien lo es, y se dice:
 * el envolvente es una aproximacion (el ataque es exponencial con el tiempo
 * de la hoja, no el contador del chip), y se genera directamente a la
 * frecuencia de salida en vez de a los 49.716 Hz del chip.
 *
 * == Lo que tiene de mas que el chip, y por que ==
 *
 * DIECIOCHO canales en vez de nueve: dos bancos de registros, como el OPL3
 * pero sin sus extras (ni cuatro operadores ni estereo). La musica de DOOM
 * llega a 15 notas a la vez (D_INTRO) y el chip de 1993 las robaba; aqui no
 * hay por que.
 *
 * == Lo que NO tiene ==
 *
 * El modo ritmo (0xBD bits 0..5): DMX no lo usa, la percusion de DOOM son
 * instrumentos melodicos del GENMIDI. Y la seleccion de nota (0x08).
 *
 * Sin DOOM dentro: solo enteros, y se compila igual con BMO C (para DOOM) que
 * con cl en Windows (`doom-port/musica_host.c`, que lo oye antes del metal). */

#include "bmo_opl_tablas.c"

#define BMO_OPL_CANALES 18
/* El reloj del chip: la frecuencia a la que el YM3812 hace sus cuentas. */
#define BMO_OPL_RELOJ 49716

#define BMO_EG_APAGADO 0
#define BMO_EG_ATAQUE 1
#define BMO_EG_CAIDA 2
#define BMO_EG_SOSTEN 3
#define BMO_EG_SUELTA 4

/* El silencio del envolvente: 511 unidades de 0,1875 dB, en Q16. */
#define BMO_EG_TOPE (511 << 16)

typedef struct {
    /* 0x20: tremolo, vibrato, sostiene (EG-TYP), KSR y el multiplo (x2). */
    int am;
    int vib;
    int sostiene;
    int ksr;
    int mult2;
    /* 0x40: KSL y nivel total (TL, pasos de 0,75 dB). */
    int ksl;
    int tl;
    /* 0x60 y 0x80: ataque, caida, nivel de sosten y suelta. */
    int ar;
    int dr;
    int sl;
    int rr;
    /* 0xE0: la onda. */
    int onda;
    /* La fase: 2^32 por ciclo. Se mira solo `>> 22`, asi que lo que desborde
     * por encima no importa. */
    unsigned long long fase;
    unsigned long long paso;
    /* La atenuacion del envolvente, Q16 de unidades de 0,1875 dB. */
    int eg;
    int estado;
    /* Lo que el envolvente avanza por muestra, ya con la KSR: -1 = ataque al
     * instante. */
    int k_ataque;
    int paso_caida;
    int paso_suelta;
    int ksl_u;
    /* Las dos ultimas salidas: la realimentacion de la moduladora. */
    int sal0;
    int sal1;
} BMO_OPL_OP;

typedef struct {
    int fnum;
    int bloque;
    int tecla;
    int fb;
    int cnt;
    BMO_OPL_OP op[2];
} BMO_OPL_CANAL;

static BMO_OPL_CANAL bmo_opl_c[BMO_OPL_CANALES];
static int bmo_opl_hz;
static int bmo_opl_wse;
static int bmo_opl_am_hondo;
static int bmo_opl_vib_hondo;
static unsigned long long bmo_opl_lfo_am;
static unsigned long long bmo_opl_lfo_vib;
static unsigned long long bmo_opl_paso_am;
static unsigned long long bmo_opl_paso_vib;
/* Muestras que la suma saco de 16 bits y se sujetaron. Tiene que ser CERO o
 * casi: si crece, la musica sale mas fuerte de lo que cabe. */
static unsigned long long bmo_opl_recortes;

/* **Las cuatro ondas, ya hechas** (4 x 1024): mirar una tabla es mas barato
 * que decidir la onda en cada muestra. Las rellena `bmo_opl_iniciar`. */
static int bmo_opl_ondas[4 * 1024];

/* Cada cuantas muestras se mueven el envolvente y los LFO. El chip los mueve a
 * su reloj; aqui, cada 4 muestras de 24 kHz (0,17 ms): no se oye, y es lo que
 * separa 31 fps de 57 en DOOM (2026-09-22, el metal lo dijo). */
#define BMO_OPL_BLOQUE 4

/* El multiplo de frecuencia de cada operador, por dos (el 0 vale 1/2). */
static const int bmo_opl_mult2[16] = {1, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 20, 24, 24, 30, 30};

/* ===================================================================
 *  Las cuentas que dependen de la nota: fase, KSL, tiempos
 * =================================================================== */

/* El indice de velocidad (0..63) de un registro de 4 bits, con la KSR. */
static int bmo_opl_indice(int r, int kso) {
    int i;
    if (r == 0) {
        return 0;
    }
    i = r * 4 + kso;
    if (i > 63) {
        i = 63;
    }
    return i;
}

/* **El ataque.** La hoja del chip da el tiempo de 0 a pleno: 2.826 ms con
 * AR=1, la mitad por cada escalon, y los cuatro sub-escalones de la KSR van
 * como 4/(4+n). Aqui el ataque es exponencial (la atenuacion baja un tanto
 * por muestra) con esa misma duracion: 6,93 constantes de tiempo en el
 * tiempo de la hoja. Devuelve Q16 por muestra; -1 es al instante. */
static int bmo_opl_k_ataque(int idx) {
    unsigned long long k;
    int r;
    int rof;

    if (idx == 0) {
        return 0;
    }
    if (idx >= 60) {
        return -1;
    }
    r = idx >> 2;
    rof = idx & 3;
    /* 40.174 = 6,93 x 65.536 / (4 x 2,82624): la cuenta de arriba, entera. */
    k = ((unsigned long long)40174 * (unsigned long long)(4 + rof)) << (r - 1);
    k = k / (unsigned long long)bmo_opl_hz;
    if (k >= 65536) {
        return -1;
    }
    if (k == 0) {
        k = 1;
    }
    return (int)k;
}

/* **La caida y la suelta**: lineales en dB, como el chip. 96 dB en 39.280 ms
 * con DR=1, la mitad por escalon, 4/(4+n) por sub-escalon. Q16 por muestra. */
static int bmo_opl_paso_lineal(int idx) {
    unsigned long long num;
    unsigned long long den;
    unsigned long long p;
    int r;
    int rof;

    if (idx == 0) {
        return 0;
    }
    r = idx >> 2;
    rof = idx & 3;
    /* 512 unidades en Q16 = 2^25; 157,123 = 4 x 39,28064 s. */
    num = (unsigned long long)(4 + rof) << (r - 1 + 25);
    den = (unsigned long long)157123 * (unsigned long long)bmo_opl_hz / 1000;
    p = num / den;
    if (p > BMO_EG_TOPE) {
        p = BMO_EG_TOPE;
    }
    if (p == 0) {
        p = 1;
    }
    return (int)p;
}

/* Rehace todo lo que depende de la nota y de los registros de un canal. */
static void bmo_opl_rehacer(BMO_OPL_CANAL *c) {
    int ks;
    int base;
    int i;
    BMO_OPL_OP *o;

    ks = (c->bloque << 1) | ((c->fnum >> 9) & 1);
    base = (bmo_opl_kslrom[c->fnum >> 6] << 2) - ((8 - c->bloque) << 5);
    if (base < 0) {
        base = 0;
    }
    for (i = 0; i < 2; i++) {
        int kso;
        o = &c->op[i];
        kso = o->ksr ? ks : (ks >> 2);
        o->k_ataque = bmo_opl_k_ataque(bmo_opl_indice(o->ar, kso));
        o->paso_caida = bmo_opl_paso_lineal(bmo_opl_indice(o->dr, kso));
        o->paso_suelta = bmo_opl_paso_lineal(bmo_opl_indice(o->rr, kso));
        /* KSL 0 no atenua; 1 = 3 dB/oct, 2 = 1,5 dB/oct, 3 = 6 dB/oct. */
        if (o->ksl == 0) {
            o->ksl_u = 0;
        } else if (o->ksl == 1) {
            o->ksl_u = base >> 1;
        } else if (o->ksl == 2) {
            o->ksl_u = base >> 2;
        } else {
            o->ksl_u = base;
        }
        /* La fase: F = fnum x 49.716 x 2^bloque / 2^20, por el multiplo, en
         * 2^32 por ciclo y por muestra de salida. */
        o->paso = ((unsigned long long)c->fnum * (unsigned long long)o->mult2
                   * (unsigned long long)BMO_OPL_RELOJ) << (c->bloque + 11);
        o->paso = o->paso / (unsigned long long)bmo_opl_hz;
    }
}

/* ===================================================================
 *  Los registros
 * =================================================================== */

/* **Poner el chip a cero** a `hz` muestras por segundo. */
void bmo_opl_iniciar(int hz) {
    int i;
    int j;

    bmo_opl_hz = hz > 0 ? hz : 24000;
    for (i = 0; i < BMO_OPL_CANALES; i++) {
        BMO_OPL_CANAL *c = &bmo_opl_c[i];
        c->fnum = 0;
        c->bloque = 0;
        c->tecla = 0;
        c->fb = 0;
        c->cnt = 0;
        for (j = 0; j < 2; j++) {
            BMO_OPL_OP *o = &c->op[j];
            o->am = 0;
            o->vib = 0;
            o->sostiene = 0;
            o->ksr = 0;
            o->mult2 = 1;
            o->ksl = 0;
            o->tl = 63;
            o->ar = 0;
            o->dr = 0;
            o->sl = 0;
            o->rr = 0;
            o->onda = 0;
            o->fase = 0;
            o->paso = 0;
            o->eg = BMO_EG_TOPE;
            o->estado = BMO_EG_APAGADO;
            o->sal0 = 0;
            o->sal1 = 0;
        }
        bmo_opl_rehacer(c);
    }
    bmo_opl_wse = 0;
    bmo_opl_am_hondo = 0;
    bmo_opl_vib_hondo = 0;
    bmo_opl_lfo_am = 0;
    bmo_opl_lfo_vib = 0;
    /* El tremolo va a 3,7 Hz y el vibrato a 6,07 Hz: los del chip. */
    bmo_opl_paso_am = ((unsigned long long)37 << 32) / ((unsigned long long)10 * bmo_opl_hz);
    bmo_opl_paso_vib = ((unsigned long long)607 << 32) / ((unsigned long long)100 * bmo_opl_hz);
    bmo_opl_recortes = 0;
    for (i = 0; i < 1024; i++) {
        j = i & 511;
        bmo_opl_ondas[i] = bmo_opl_seno[i];
        bmo_opl_ondas[1024 + i] = i < 512 ? bmo_opl_seno[i] : 0;
        bmo_opl_ondas[2048 + i] = bmo_opl_seno[j];
        bmo_opl_ondas[3072 + i] = j < 256 ? bmo_opl_seno[j] : 0;
    }
}

/* **Escribir un registro.** `reg` va de 0x000 a 0x1FF: el bit 8 es el banco
 * (canales 0..8 o 9..17). Lo que no es un registro de este chip se ignora,
 * como lo ignoraria el chip. */
void bmo_opl_escribir(int reg, int val) {
    int banco;
    int r;
    int grupo;
    int s;
    int canal;
    BMO_OPL_CANAL *c;
    BMO_OPL_OP *o;

    banco = (reg >> 8) & 1;
    r = reg & 0xFF;
    val = val & 0xFF;

    if (r == 0x01) {
        if (banco == 0) {
            bmo_opl_wse = (val >> 5) & 1;
        }
        return;
    }
    if (r == 0xBD) {
        if (banco == 0) {
            bmo_opl_am_hondo = (val >> 7) & 1;
            bmo_opl_vib_hondo = (val >> 6) & 1;
        }
        return;
    }
    if (r >= 0xA0 && r <= 0xC8 && (r & 0x0F) <= 8) {
        canal = banco * 9 + (r & 0x0F);
        c = &bmo_opl_c[canal];
        if ((r & 0xF0) == 0xA0) {
            c->fnum = (c->fnum & 0x300) | val;
            bmo_opl_rehacer(c);
        } else if ((r & 0xF0) == 0xB0) {
            int tecla;
            int j;
            c->fnum = (c->fnum & 0xFF) | ((val & 3) << 8);
            c->bloque = (val >> 2) & 7;
            tecla = (val >> 5) & 1;
            bmo_opl_rehacer(c);
            if (tecla && !c->tecla) {
                /* Pulsar: los dos operadores atacan DESDE donde esten (el
                 * chip no vuelve al silencio) y la fase vuelve a cero. */
                for (j = 0; j < 2; j++) {
                    c->op[j].estado = BMO_EG_ATAQUE;
                    c->op[j].fase = 0;
                }
            } else if (!tecla && c->tecla) {
                for (j = 0; j < 2; j++) {
                    if (c->op[j].estado != BMO_EG_APAGADO) {
                        c->op[j].estado = BMO_EG_SUELTA;
                    }
                }
            }
            c->tecla = tecla;
        } else if ((r & 0xF0) == 0xC0) {
            c->fb = (val >> 1) & 7;
            c->cnt = val & 1;
        }
        return;
    }

    /* Los registros de OPERADOR: 0x20, 0x40, 0x60, 0x80 y 0xE0, con 22
     * direcciones cada uno de las que 18 son operadores. */
    if (!((r >= 0x20 && r <= 0x95) || (r >= 0xE0 && r <= 0xF5))) {
        return;
    }
    grupo = (r & 0x1F) >> 3;
    s = r & 7;
    if (s >= 6 || grupo > 2) {
        return;
    }
    canal = banco * 9 + grupo * 3 + (s % 3);
    c = &bmo_opl_c[canal];
    o = &c->op[s / 3];

    switch (r & 0xE0) {
    case 0x20:
        o->am = (val >> 7) & 1;
        o->vib = (val >> 6) & 1;
        o->sostiene = (val >> 5) & 1;
        o->ksr = (val >> 4) & 1;
        o->mult2 = bmo_opl_mult2[val & 15];
        break;
    case 0x40:
        o->ksl = (val >> 6) & 3;
        o->tl = val & 63;
        break;
    case 0x60:
        o->ar = (val >> 4) & 15;
        o->dr = val & 15;
        break;
    case 0x80:
        o->sl = (val >> 4) & 15;
        o->rr = val & 15;
        break;
    case 0xE0:
        o->onda = val & 3;
        break;
    default:
        return;
    }
    bmo_opl_rehacer(c);
}

/* ===================================================================
 *  Generar
 * =================================================================== */

/* `pasos` pasos del envolvente de un operador, de una vez. */
static void bmo_opl_envolvente(BMO_OPL_OP *o, int pasos) {
    int sl;

    switch (o->estado) {
    case BMO_EG_ATAQUE:
        if (o->k_ataque < 0) {
            o->eg = 0;
        } else if (o->k_ataque > 0) {
            long long baja = ((long long)o->eg * (long long)o->k_ataque * (long long)pasos) >> 16;
            if (baja >= o->eg) {
                o->eg = 0;
            } else {
                o->eg = o->eg - (int)baja;
            }
        }
        if (o->eg < (1 << 16)) {
            o->eg = 0;
            o->estado = BMO_EG_CAIDA;
        }
        break;
    case BMO_EG_CAIDA:
        /* SL=15 son 93 dB, no 45: el chip salta el escalon. */
        sl = (o->sl == 15 ? 31 : o->sl) * 16;
        o->eg = o->eg + o->paso_caida * pasos;
        if (o->eg >= (sl << 16)) {
            o->eg = sl << 16;
            o->estado = BMO_EG_SOSTEN;
        }
        break;
    case BMO_EG_SOSTEN:
        /* Sin EG-TYP (un sonido de percusion), aun con la tecla pulsada
         * sigue cayendo, a la velocidad de la suelta. */
        if (!o->sostiene) {
            o->eg = o->eg + o->paso_suelta * pasos;
            if (o->eg >= BMO_EG_TOPE) {
                o->eg = BMO_EG_TOPE;
                o->estado = BMO_EG_APAGADO;
            }
        }
        break;
    case BMO_EG_SUELTA:
        o->eg = o->eg + o->paso_suelta * pasos;
        if (o->eg >= BMO_EG_TOPE) {
            o->eg = BMO_EG_TOPE;
            o->estado = BMO_EG_APAGADO;
        }
        break;
    default:
        o->eg = BMO_EG_TOPE;
        break;
    }
}

/* La ganancia de un operador para el bloque, en Q15: el envolvente, el nivel,
 * la KSL y el tremolo. 0 = no suena. */
static int bmo_opl_ganancia(BMO_OPL_OP *o, int trem) {
    int att = (o->eg >> 16) + (o->tl << 2) + o->ksl_u + (o->am ? trem : 0);
    if (att >= 511) {
        return 0;
    }
    return bmo_opl_exp[att];
}

/* El paso de fase del bloque, con el vibrato si lo lleva: `vib` es una
 * fraccion Q16 con signo (+-265 o +-530: 7 o 14 cents). */
static unsigned long long bmo_opl_paso(BMO_OPL_OP *o, int vib) {
    unsigned long long p = o->paso;
    if (o->vib) {
        if (vib >= 0) {
            p = p + ((p * (unsigned long long)vib) >> 16);
        } else {
            p = p - ((p * (unsigned long long)(-vib)) >> 16);
        }
    }
    return p;
}

/* **Generar `n` muestras MONO** de 16 bits en `dst`.
 *
 * Por bloques de [`BMO_OPL_BLOQUE`]: el envolvente, el tremolo y el vibrato
 * se calculan UNA vez por bloque, y dentro solo queda lo que cambia en cada
 * muestra -- la fase, la tabla y la multiplicacion. Sin llamadas dentro. */
void bmo_opl_generar(int *dst, int n) {
    int k = 0;

    while (k < n) {
        int m = n - k;
        int p;
        int trem;
        int vib;
        int j;
        int ci;

        if (m > BMO_OPL_BLOQUE) {
            m = BMO_OPL_BLOQUE;
        }
        /* Los dos LFO, en triangulo, avanzados el bloque entero. */
        bmo_opl_lfo_am = bmo_opl_lfo_am + bmo_opl_paso_am * (unsigned long long)m;
        bmo_opl_lfo_vib = bmo_opl_lfo_vib + bmo_opl_paso_vib * (unsigned long long)m;
        p = (int)((bmo_opl_lfo_am >> 16) & 0xFFFF);
        p = p < 32768 ? p : 65535 - p;
        /* 4,8 dB son 25,6 unidades; 1 dB, 5,3. */
        trem = ((bmo_opl_am_hondo ? 26 : 5) * p) >> 15;
        p = (int)((bmo_opl_lfo_vib >> 16) & 0xFFFF);
        p = p < 32768 ? p - 16384 : 49151 - p;
        vib = ((bmo_opl_vib_hondo ? 530 : 265) * p) / 16384;

        for (j = 0; j < m; j++) {
            dst[k + j] = 0;
        }
        for (ci = 0; ci < BMO_OPL_CANALES; ci++) {
            BMO_OPL_CANAL *c = &bmo_opl_c[ci];
            BMO_OPL_OP *mo = &c->op[0];
            BMO_OPL_OP *q = &c->op[1];
            unsigned long long fm;
            unsigned long long fc;
            unsigned long long pm;
            unsigned long long pc;
            int gm;
            int gc;
            int bm;
            int bc;
            int s0;
            int s1;
            int fb;

            /* Un canal callado no cuesta nada. En FM solo suena la portadora;
             * sumando, las dos. */
            if (q->estado == BMO_EG_APAGADO && (c->cnt == 0 || mo->estado == BMO_EG_APAGADO)) {
                continue;
            }
            bmo_opl_envolvente(mo, m);
            bmo_opl_envolvente(q, m);
            gm = bmo_opl_ganancia(mo, trem);
            gc = bmo_opl_ganancia(q, trem);
            pm = bmo_opl_paso(mo, vib);
            pc = bmo_opl_paso(q, vib);
            bm = bmo_opl_wse ? (mo->onda << 10) : 0;
            bc = bmo_opl_wse ? (q->onda << 10) : 0;
            fm = mo->fase;
            fc = q->fase;
            s0 = mo->sal0;
            s1 = mo->sal1;
            fb = c->fb;
            for (j = 0; j < m; j++) {
                int realim = fb ? ((s0 + s1) >> (9 - fb)) : 0;
                int sm = (bmo_opl_ondas[bm + (((int)(fm >> 22) + realim) & 1023)] * gm) >> 15;
                int v;
                fm = fm + pm;
                s1 = s0;
                s0 = sm;
                if (c->cnt == 0) {
                    v = (bmo_opl_ondas[bc + (((int)(fc >> 22) + sm) & 1023)] * gc) >> 15;
                } else {
                    v = sm + ((bmo_opl_ondas[bc + ((int)(fc >> 22) & 1023)] * gc) >> 15);
                }
                fc = fc + pc;
                dst[k + j] = dst[k + j] + v;
            }
            mo->fase = fm;
            q->fase = fc;
            mo->sal0 = s0;
            mo->sal1 = s1;
        }
        for (j = 0; j < m; j++) {
            /* ** LA GANANCIA DE SALIDA, x1,75, MEDIDA: un operador del chip da
             * +-4.095 (13 bits), asi que una suma de pocas voces se queda muy
             * por debajo de 16 bits. El pico mas alto de las 13 canciones de
             * doom1.wad es 17.448 (D_INTROA); x1,75 lo deja en 30.534. Un WAD
             * con musica mas densa se SUJETA aqui, y `bmo_opl_recortes` lo
             * cuenta. */
            int suma = (dst[k + j] * 7) >> 2;
            if (suma > 32767) {
                suma = 32767;
                bmo_opl_recortes++;
            } else if (suma < -32767) {
                suma = -32767;
                bmo_opl_recortes++;
            }
            dst[k + j] = suma;
        }
        k = k + m;
    }
}
