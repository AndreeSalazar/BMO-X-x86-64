/* bmo_mus.c -- EL REPRODUCTOR DE MUS: la partitura de DOOM sobre el chip FM.
 *
 * DOOM guarda su musica como MUS (una partitura de eventos a 140 tics por
 * segundo) y sus instrumentos en el lump GENMIDI (175 instrumentos FM de dos
 * operadores: 128 melodicos y 47 de percusion). Este fichero lee lo uno y
 * toca lo otro sobre `bmo_opl.c` escribiendole registros, que es lo que
 * hacia la libreria DMX con la Sound Blaster en 1993.
 *
 * == Escrito para BMO-X, no copiado ==
 *
 * El formato MUS y el del GENMIDI son publicos (los documentan las propias
 * herramientas de DOOM desde 1994). El reparto de voces, la curva de volumen
 * y la cuenta de la nota son propios, y donde DMX hacia otra cosa se dice:
 *
 *   - VOCES: 18 y no 9. La mas vieja se ROBA solo si no queda ninguna.
 *   - VOLUMEN: la curva de General MIDI, 40 log10(v/127), y no la tabla de
 *     DMX (que no se ha copiado de ningun sitio).
 *   - SE IGNORAN: paneo (el chip es mono), modulacion, expresion, pedal.
 *
 * Sin DOOM dentro: se compila igual con BMO C y con cl en Windows. */

#include "bmo_opl.c"

#define BMO_MUS_VOCES 18
#define BMO_MUS_TICS 140
#define BMO_MUS_INSTRUMENTOS 175
/* Un instrumento del GENMIDI mide 36 bytes, detras de 8 de firma. */
#define BMO_MUS_GM_INSTR 36
#define BMO_MUS_GM_FIJA 0x0001
#define BMO_MUS_GM_DOBLE 0x0004

/* Donde van la moduladora de cada canal del chip; la portadora, 3 mas alla. */
static const int bmo_mus_op[9] = {0x00, 0x01, 0x02, 0x08, 0x09, 0x0A, 0x10, 0x11, 0x12};

typedef struct {
    int programa;
    int volumen;
    /* La rueda de tono, en 1/32 de semitono: -64..63 son +-2 semitonos. */
    int rueda;
    /* El volumen de la ultima nota: una nota sin volumen repite el anterior. */
    int ultima_vel;
} BMO_MUS_CANAL;

typedef struct {
    int pulsada;
    int canal;
    int tecla;
    int instr;
    /* Que voz del instrumento: 0, o 1 para la segunda de uno DOBLE. */
    int iv;
    int vel;
    /* Cuando se pulso o se solto: para elegir la mas vieja. */
    unsigned int edad;
} BMO_MUS_VOZ;

static const unsigned char *bmo_mus_gm;
static const unsigned char *bmo_mus_dato;
static int bmo_mus_largo;
static int bmo_mus_pos;
static int bmo_mus_espera;
static int bmo_mus_fin;
static int bmo_mus_hz;
static unsigned long long bmo_mus_tic;
static int bmo_mus_en_tic;
static unsigned int bmo_mus_reloj;
static BMO_MUS_CANAL bmo_mus_can[16];
static BMO_MUS_VOZ bmo_mus_voz[BMO_MUS_VOCES];
/* Notas que no tuvieron voz libre y robaron una: la medida de si 18 bastan. */
static unsigned int bmo_mus_robadas;

/* ===================================================================
 *  El GENMIDI
 * =================================================================== */

/* **Cargar los instrumentos.** Devuelve 1 si el lump es un GENMIDI entero. */
int bmo_mus_instrumentos(const unsigned char *lump, int largo) {
    if (lump == 0 || largo < 8 + BMO_MUS_INSTRUMENTOS * BMO_MUS_GM_INSTR) {
        return 0;
    }
    if (lump[0] != '#' || lump[1] != 'O' || lump[2] != 'P' || lump[3] != 'L'
        || lump[4] != '_' || lump[5] != 'I' || lump[6] != 'I' || lump[7] != '#') {
        return 0;
    }
    bmo_mus_gm = lump + 8;
    return 1;
}

static int bmo_mus_gm_flags(int i) {
    const unsigned char *p = bmo_mus_gm + i * BMO_MUS_GM_INSTR;
    return p[0] | (p[1] << 8);
}

/* La voz `iv` del instrumento `i`: 16 bytes -- moduladora (6), realimentacion,
 * portadora (6), relleno y el desplazamiento de nota (16 bits con signo). */
static const unsigned char *bmo_mus_gm_voz(int i, int iv) {
    return bmo_mus_gm + i * BMO_MUS_GM_INSTR + 4 + iv * 16;
}

static int bmo_mus_gm_desplazamiento(const unsigned char *v) {
    int d = v[14] | (v[15] << 8);
    if (d >= 32768) {
        d = d - 65536;
    }
    return d;
}

/* ===================================================================
 *  Las voces: registros del chip
 * =================================================================== */

static int bmo_mus_reg(int v, int base, int portadora) {
    int banco = v / 9;
    int cc = v % 9;
    return (banco << 8) | (base + bmo_mus_op[cc] + (portadora ? 3 : 0));
}

static int bmo_mus_reg_canal(int v, int base) {
    return ((v / 9) << 8) | (base + (v % 9));
}

/* **El volumen** de una voz: la curva de la nota MAS la del canal, en pasos de
 * 0,75 dB, sobre el nivel que trae el instrumento. En FM solo se oye la
 * portadora; sumando, tambien la moduladora. */
static void bmo_mus_volumen(int v) {
    BMO_MUS_VOZ *z = &bmo_mus_voz[v];
    const unsigned char *d = bmo_mus_gm_voz(z->instr, z->iv);
    int att;
    int car;
    int mod;

    att = bmo_mus_db[z->vel & 127] + bmo_mus_db[bmo_mus_can[z->canal].volumen & 127];
    car = (d[12] & 63) + att;
    if (car > 63) {
        car = 63;
    }
    bmo_opl_escribir(bmo_mus_reg(v, 0x40, 1), (d[11] & 0xC0) | car);
    if (d[6] & 1) {
        mod = (d[5] & 63) + att;
        if (mod > 63) {
            mod = 63;
        }
        bmo_opl_escribir(bmo_mus_reg(v, 0x40, 0), (d[4] & 0xC0) | mod);
    }
}

/* **La nota** de una voz a los registros A0/B0, con la tecla como diga. */
static void bmo_mus_frecuencia(int v, int pulsar) {
    BMO_MUS_VOZ *z = &bmo_mus_voz[v];
    const unsigned char *d = bmo_mus_gm_voz(z->instr, z->iv);
    int nota;
    int p;
    int oct;
    int frac;
    int b;
    unsigned long long f;
    unsigned long long fnum;

    if (bmo_mus_gm_flags(z->instr) & BMO_MUS_GM_FIJA) {
        nota = bmo_mus_gm[z->instr * BMO_MUS_GM_INSTR + 3];
    } else {
        nota = z->tecla + bmo_mus_gm_desplazamiento(d);
    }
    /* Fuera del teclado se dobla por octavas, como DMX: una nota imposible
     * suena en su octava posible, no pegada al borde. */
    while (nota < 0) {
        nota = nota + 12;
    }
    while (nota > 95) {
        nota = nota - 12;
    }
    /* En 1/32 de semitono, con la rueda y, en la segunda voz de un DOBLE, su
     * afinacion fina (128 = ninguna). */
    p = nota * 32 + bmo_mus_can[z->canal].rueda;
    if (z->iv == 1) {
        p = p + (bmo_mus_gm[z->instr * BMO_MUS_GM_INSTR + 2] >> 1) - 64;
    }
    /* La frecuencia en Hz Q16: 440 x 2^((p - LA) / 384).
     *
     * ** Y EL LA ES LA NOTA 57, NO LA 69. DMX numera una octava POR ENCIMA
     * del MIDI de siempre: la primera fila de su tabla de frecuencias es un
     * F-Number de 0x133 (14,56 Hz), que solo cuadra con esa octava de mas. Y
     * los instrumentos del GENMIDI lo compensan: 72 de las 128 portadoras
     * llevan multiplo x0,5. Con 69 toda la musica sonaria una octava grave. */
    p = p - 57 * 32;
    oct = p >= 0 ? p / 384 : -((-p + 383) / 384);
    frac = p - oct * 384;
    f = ((unsigned long long)440 << 16) * (unsigned long long)bmo_opl_pot2[frac];
    f = f >> 30;
    if (oct >= 0) {
        f = f << oct;
    } else {
        f = f >> (-oct);
    }
    /* El bloque mas bajo en el que el F-Number cabe en 10 bits: la mayor
     * resolucion. F-Number = F x 2^(20 - bloque) / 49.716. */
    fnum = 1023;
    for (b = 0; b < 8; b++) {
        fnum = (f << (20 - b)) / ((unsigned long long)BMO_OPL_RELOJ << 16);
        if (fnum <= 1023) {
            break;
        }
    }
    if (b > 7) {
        b = 7;
        fnum = 1023;
    }
    bmo_opl_escribir(bmo_mus_reg_canal(v, 0xA0), (int)(fnum & 0xFF));
    bmo_opl_escribir(bmo_mus_reg_canal(v, 0xB0),
                     (pulsar ? 0x20 : 0) | (b << 2) | (int)((fnum >> 8) & 3));
}

/* **Cargar el instrumento** en los registros de la voz. */
static void bmo_mus_cargar(int v) {
    BMO_MUS_VOZ *z = &bmo_mus_voz[v];
    const unsigned char *d = bmo_mus_gm_voz(z->instr, z->iv);

    bmo_opl_escribir(bmo_mus_reg(v, 0x20, 0), d[0]);
    bmo_opl_escribir(bmo_mus_reg(v, 0x60, 0), d[1]);
    bmo_opl_escribir(bmo_mus_reg(v, 0x80, 0), d[2]);
    bmo_opl_escribir(bmo_mus_reg(v, 0xE0, 0), d[3]);
    bmo_opl_escribir(bmo_mus_reg(v, 0x40, 0), (d[4] & 0xC0) | (d[5] & 63));
    bmo_opl_escribir(bmo_mus_reg(v, 0x20, 1), d[7]);
    bmo_opl_escribir(bmo_mus_reg(v, 0x60, 1), d[8]);
    bmo_opl_escribir(bmo_mus_reg(v, 0x80, 1), d[9]);
    bmo_opl_escribir(bmo_mus_reg(v, 0xE0, 1), d[10]);
    bmo_opl_escribir(bmo_mus_reg_canal(v, 0xC0), d[6]);
    bmo_mus_volumen(v);
}

/* **Una voz libre**: la que se solto hace mas tiempo; si todas suenan, se
 * ROBA la mas vieja, y se cuenta. */
static int bmo_mus_libre(void) {
    int v;
    int mejor = -1;
    int vieja = 0;

    for (v = 0; v < BMO_MUS_VOCES; v++) {
        if (!bmo_mus_voz[v].pulsada) {
            if (mejor < 0 || bmo_mus_voz[v].edad < bmo_mus_voz[mejor].edad) {
                mejor = v;
            }
        }
        if (bmo_mus_voz[v].edad < bmo_mus_voz[vieja].edad) {
            vieja = v;
        }
    }
    if (mejor >= 0) {
        return mejor;
    }
    bmo_mus_robadas++;
    bmo_mus_voz[vieja].pulsada = 0;
    bmo_mus_frecuencia(vieja, 0);
    return vieja;
}

static void bmo_mus_pulsar(int canal, int tecla, int vel) {
    int instr;
    int voces;
    int iv;

    if (canal == 15) {
        /* La percusion: una nota es un instrumento, del 35 al 81. */
        if (tecla < 35 || tecla > 81) {
            return;
        }
        instr = 128 + tecla - 35;
    } else {
        instr = bmo_mus_can[canal].programa & 127;
    }
    voces = (bmo_mus_gm_flags(instr) & BMO_MUS_GM_DOBLE) ? 2 : 1;
    for (iv = 0; iv < voces; iv++) {
        int v = bmo_mus_libre();
        BMO_MUS_VOZ *z = &bmo_mus_voz[v];
        z->pulsada = 1;
        z->canal = canal;
        z->tecla = tecla;
        z->instr = instr;
        z->iv = iv;
        z->vel = vel;
        z->edad = ++bmo_mus_reloj;
        bmo_mus_cargar(v);
        bmo_mus_frecuencia(v, 1);
    }
}

static void bmo_mus_soltar(int canal, int tecla) {
    int v;

    for (v = 0; v < BMO_MUS_VOCES; v++) {
        BMO_MUS_VOZ *z = &bmo_mus_voz[v];
        if (z->pulsada && z->canal == canal && (tecla < 0 || z->tecla == tecla)) {
            z->pulsada = 0;
            z->edad = ++bmo_mus_reloj;
            bmo_mus_frecuencia(v, 0);
        }
    }
}

/* ===================================================================
 *  La partitura
 * =================================================================== */

static int bmo_mus_byte(void) {
    if (bmo_mus_pos >= bmo_mus_largo) {
        bmo_mus_fin = 1;
        return 0;
    }
    return bmo_mus_dato[bmo_mus_pos++];
}

/* El retardo que sigue a un grupo de eventos: 7 bits por byte. */
static int bmo_mus_retardo(void) {
    int t = 0;
    int x;
    int n = 0;

    do {
        x = bmo_mus_byte();
        t = t * 128 + (x & 127);
        n++;
    } while ((x & 0x80) && !bmo_mus_fin && n < 5);
    return t;
}

/* **Un grupo de eventos**, hasta el que lleva el retardo. `tocar` = 0 solo
 * cuenta (para medir la cancion). */
static void bmo_mus_grupo(int tocar) {
    int v;

    for (;;) {
        int e = bmo_mus_byte();
        int tipo = (e >> 4) & 7;
        int canal = e & 15;
        int a;
        int b;
        int k;

        if (bmo_mus_fin) {
            return;
        }
        switch (tipo) {
        case 0:
            a = bmo_mus_byte();
            if (tocar) {
                bmo_mus_soltar(canal, a & 127);
            }
            break;
        case 1:
            a = bmo_mus_byte();
            if (a & 0x80) {
                bmo_mus_can[canal].ultima_vel = bmo_mus_byte() & 127;
            }
            if (tocar) {
                bmo_mus_pulsar(canal, a & 127, bmo_mus_can[canal].ultima_vel);
            }
            break;
        case 2:
            a = bmo_mus_byte();
            bmo_mus_can[canal].rueda = (a >> 1) - 64;
            if (tocar) {
                for (v = 0; v < BMO_MUS_VOCES; v++) {
                    if (bmo_mus_voz[v].pulsada && bmo_mus_voz[v].canal == canal) {
                        bmo_mus_frecuencia(v, 1);
                    }
                }
            }
            break;
        case 3:
            a = bmo_mus_byte();
            if (tocar && (a == 10 || a == 11)) {
                bmo_mus_soltar(canal, -1);
            }
            if (a == 14) {
                bmo_mus_can[canal].volumen = 100;
                bmo_mus_can[canal].rueda = 0;
            }
            break;
        case 4:
            a = bmo_mus_byte();
            b = bmo_mus_byte() & 127;
            if (a == 0) {
                bmo_mus_can[canal].programa = b;
            } else if (a == 3) {
                bmo_mus_can[canal].volumen = b;
                if (tocar) {
                    for (k = 0; k < BMO_MUS_VOCES; k++) {
                        if (bmo_mus_voz[k].pulsada && bmo_mus_voz[k].canal == canal) {
                            bmo_mus_volumen(k);
                        }
                    }
                }
            }
            break;
        case 5:
            break;
        case 6:
            bmo_mus_fin = 1;
            return;
        default:
            /* Un tipo que no existe: la partitura esta rota. Se acaba aqui
             * en vez de adivinar cuantos bytes lleva. */
            bmo_mus_fin = 1;
            return;
        }
        if (e & 0x80) {
            bmo_mus_espera = bmo_mus_retardo();
            return;
        }
    }
}

static void bmo_mus_a_cero(void) {
    int i;

    for (i = 0; i < 16; i++) {
        bmo_mus_can[i].programa = 0;
        bmo_mus_can[i].volumen = 100;
        bmo_mus_can[i].rueda = 0;
        bmo_mus_can[i].ultima_vel = 127;
    }
    for (i = 0; i < BMO_MUS_VOCES; i++) {
        bmo_mus_voz[i].pulsada = 0;
        bmo_mus_voz[i].edad = 0;
        bmo_mus_voz[i].instr = 0;
        bmo_mus_voz[i].iv = 0;
        bmo_mus_voz[i].canal = 0;
        bmo_mus_voz[i].tecla = 0;
        bmo_mus_voz[i].vel = 0;
    }
    bmo_mus_reloj = 0;
    bmo_mus_robadas = 0;
    bmo_mus_espera = 0;
    bmo_mus_fin = 0;
    bmo_mus_tic = 0;
    bmo_mus_en_tic = 0;
}

/* **Empezar una cancion** a `hz`. Devuelve cuantas MUESTRAS mide entera (0 si
 * no es un MUS o no hay instrumentos): se sabe antes de tocar, recorriendo la
 * partitura sin sonar. */
unsigned long long bmo_mus_empezar(const unsigned char *mus, int largo, int hz) {
    int inicio;
    unsigned long long tics = 0;

    if (bmo_mus_gm == 0 || mus == 0 || largo < 16) {
        return 0;
    }
    if (mus[0] != 'M' || mus[1] != 'U' || mus[2] != 'S' || mus[3] != 0x1A) {
        return 0;
    }
    inicio = mus[6] | (mus[7] << 8);
    if (inicio >= largo) {
        return 0;
    }
    bmo_mus_dato = mus;
    bmo_mus_largo = largo;
    bmo_mus_hz = hz;

    /* Primera pasada: CONTAR. */
    bmo_mus_a_cero();
    bmo_mus_pos = inicio;
    while (!bmo_mus_fin) {
        bmo_mus_grupo(0);
        if (!bmo_mus_fin) {
            tics = tics + (unsigned long long)bmo_mus_espera;
        }
    }

    /* Y a punto para tocar. */
    bmo_mus_a_cero();
    bmo_mus_pos = inicio;
    bmo_opl_iniciar(hz);
    bmo_opl_escribir(0x01, 0x20);
    bmo_opl_escribir(0xBD, 0x00);
    return tics * (unsigned long long)hz / BMO_MUS_TICS;
}

/* **Tocar**: hasta `n` muestras en `dst`. Devuelve cuantas salieron; menos de
 * `n` es que la cancion se acabo. */
int bmo_mus_tocar(int *dst, int n) {
    int hechas = 0;

    while (hechas < n) {
        int k;
        if (bmo_mus_en_tic == 0) {
            while (bmo_mus_espera == 0 && !bmo_mus_fin) {
                bmo_mus_grupo(1);
            }
            if (bmo_mus_fin && bmo_mus_espera == 0) {
                break;
            }
            bmo_mus_espera--;
            bmo_mus_en_tic = (int)(((bmo_mus_tic + 1) * (unsigned long long)bmo_mus_hz) / BMO_MUS_TICS
                                   - (bmo_mus_tic * (unsigned long long)bmo_mus_hz) / BMO_MUS_TICS);
            bmo_mus_tic++;
            if (bmo_mus_en_tic == 0) {
                continue;
            }
        }
        k = n - hechas;
        if (k > bmo_mus_en_tic) {
            k = bmo_mus_en_tic;
        }
        bmo_opl_generar(dst + hechas, k);
        hechas = hechas + k;
        bmo_mus_en_tic = bmo_mus_en_tic - k;
    }
    return hechas;
}
