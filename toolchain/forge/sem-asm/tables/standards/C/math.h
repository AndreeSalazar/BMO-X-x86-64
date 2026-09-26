/* math.h -- la coma flotante de C en BMO-X: lo exacto, y las series.
 *
 * == Dos clases de funcion, y cada una dice lo que es ==
 *
 * LO EXACTO (`fabs`, `sqrt`, `trunc`, `floor`, `ceil`): una comparacion, un
 * signo o una instruccion de SSE2. Su resultado es el correcto bit a bit.
 *
 * LAS SERIES (26-09: `sin`, `cos`, `tan`, `atan`, `atan2`, `exp`, `log`,
 * `pow`): reduccion de rango y un polinomio, en doble y sin fusionar. NO son
 * el redondeo correcto siempre: son UNA definicion, la de BMO-X, la MISMA que
 * usa el emisor de SPIR-V. Los coeficientes estan aqui como BITS, copiados de
 * `bmo_spirv_front::math::table`, y cada funcion repite las operaciones de
 * `bmo_spirv_front::math` en el mismo orden: la prueba `serie_de_math_h` de
 * BMO C exige los mismos bits que el oraculo, y la tabla de aqui la misma que
 * la de alla. Un seno, no dos.
 *
 * Hasta el 26-09 las series no estaban, a proposito: un `sin` que devolviera
 * el argumento seria peor que un error de compilacion. Llegan con su juez.
 *
 * [!] Lo que no hacen, dicho: la reduccion de `sin`/`cos`/`tan` usa pi/2 en
 * dos trozos, buena hasta |x| ~ 2^20 (Quake no pasa de unos miles); `pow`
 * pierde precision con |y ln x| grande (unas decenas de ULP con resultados
 * enormes). Por eso la escalera al jefe final (PLAN_VERRANO 2d) las pide
 * primero para Quake por software, que mide angulos y gammas.
 */
#ifndef BMO_MATH_H
#define BMO_MATH_H

/* Valor absoluto en coma flotante.
 *
 * Se escribe con una comparacion y una negacion, y no bajando a entero: en
 * coma flotante la negacion es cambiar el bit de signo, que tambien es lo
 * correcto para el cero. */
double fabs(double v) {
    if (v < 0.0) {
        return -v;
    }
    return v;
}

float fabsf(float v) {
    if (v < 0.0) {
        return -v;
    }
    return v;
}

/* == Lo EXACTO (25-09, para Quake) ========================================
 *
 * Estas redondean UNA vez, o ninguna: su resultado es el correcto bit a bit,
 * no una aproximacion. Por eso pueden estar ya; las series (`sin`, `cos`,
 * `atan`, `pow`) esperan a la tabla de `math::table`, la misma que usa el
 * emisor de SPIR-V, para que no haya dos definiciones de un seno. */

#include <semantic/semantic.h>

/* Raiz cuadrada: `sqrtsd`, UNA instruccion de SSE2, correctamente redondeada
 * por el propio procesador (IEEE 754). */
double sqrt(double v) {
    return __sqrtsd(v);
}

float sqrtf(float v) {
    return (float)__sqrtsd(v);
}

/* Hacia cero. A partir de 2^52 un double ya es entero (y el infinito y el
 * NaN se devuelven tal cual); por debajo, cabe en un `long long`. El cero
 * conserva su signo: trunc(-0.5) es -0.0, no +0.0. */
double trunc(double v) {
    double t;
    if (v != v || v >= 4503599627370496.0 || v <= -4503599627370496.0) {
        return v;
    }
    t = (double)(long long)v;
    if (t == 0.0 && v < 0.0) {
        return -0.0;
    }
    return t;
}

/* Hacia menos infinito y hacia mas infinito, desde `trunc`. */
double floor(double v) {
    double t = trunc(v);
    if (v < t) {
        t = t - 1.0;
    }
    return t;
}

double ceil(double v) {
    double t = trunc(v);
    if (v > t) {
        t = t + 1.0;
    }
    return t;
}

float floorf(float v) {
    return (float)floor(v);
}

float ceilf(float v) {
    return (float)ceil(v);
}


/* == LAS SERIES (26-09) ====================================================
 *
 * La tabla, en BITS: asi no depende de como se redondee un literal decimal.
 * Generada desde `math::table`; si cambia alla, la prueba lo dice aqui. */

#define BMO_M_FRAC_PI_2_HI 0x3FF921FB54400000ULL
#define BMO_M_FRAC_PI_2_LO 0x3DD0B4611A626331ULL
#define BMO_M_FRAC_2_PI 0x3FE45F306DC9C883ULL
#define BMO_M_LN2_HI 0x3FE62E42FEE00000ULL
#define BMO_M_LN2_LO 0x3DEA39EF35793C76ULL
#define BMO_M_INV_LN2 0x3FF71547652B82FEULL
#define BMO_M_NEAREST 0x4338000000000000ULL
#define BMO_M_EXP_MAX 0x4086280000000000ULL
#define BMO_M_EXP_MIN 0xC087480000000000ULL
#define BMO_M_SQRT_2 0x3FF6A09E667F3BCDULL
#define BMO_M_FRAC_PI_6 0x3FE0C152382D7366ULL
#define BMO_M_SQRT_3 0x3FFBB67AE8584CAAULL
#define BMO_M_TAN_PI_12 0x3FD126145E9ECD56ULL
static const unsigned long long bmo_m_SIN[7] = {
    0x3DE6124613A86D09ULL, 0xBE5AE64567F544E4ULL, 0x3EC71DE3A556C734ULL,
    0xBF2A01A01A01A01AULL, 0x3F81111111111111ULL, 0xBFC5555555555555ULL,
    0x3FF0000000000000ULL,
};
static const unsigned long long bmo_m_COS[8] = {
    0xBDA93974A8C07C9DULL, 0x3E21EED8EFF8D898ULL, 0xBE927E4FB7789F5CULL,
    0x3EFA01A01A01A01AULL, 0xBF56C16C16C16C17ULL, 0x3FA5555555555555ULL,
    0xBFE0000000000000ULL, 0x3FF0000000000000ULL,
};
static const unsigned long long bmo_m_EXP[13] = {
    0x3E21EED8EFF8D898ULL, 0x3E5AE64567F544E4ULL, 0x3E927E4FB7789F5CULL,
    0x3EC71DE3A556C734ULL, 0x3EFA01A01A01A01AULL, 0x3F2A01A01A01A01AULL,
    0x3F56C16C16C16C17ULL, 0x3F81111111111111ULL, 0x3FA5555555555555ULL,
    0x3FC5555555555555ULL, 0x3FE0000000000000ULL, 0x3FF0000000000000ULL,
    0x3FF0000000000000ULL,
};
static const unsigned long long bmo_m_LN[11] = {
    0x3FA8618618618618ULL, 0x3FAAF286BCA1AF28ULL, 0x3FAE1E1E1E1E1E1EULL,
    0x3FB1111111111111ULL, 0x3FB3B13B13B13B14ULL, 0x3FB745D1745D1746ULL,
    0x3FBC71C71C71C71CULL, 0x3FC2492492492492ULL, 0x3FC999999999999AULL,
    0x3FD5555555555555ULL, 0x3FF0000000000000ULL,
};
static const unsigned long long bmo_m_ATAN[16] = {
    0xBFA0842108421084ULL, 0x3FA1A7B9611A7B96ULL, 0xBFA2F684BDA12F68ULL,
    0x3FA47AE147AE147BULL, 0xBFA642C8590B2164ULL, 0x3FA8618618618618ULL,
    0xBFAAF286BCA1AF28ULL, 0x3FAE1E1E1E1E1E1EULL, 0xBFB1111111111111ULL,
    0x3FB3B13B13B13B14ULL, 0xBFB745D1745D1746ULL, 0x3FBC71C71C71C71CULL,
    0xBFC2492492492492ULL, 0x3FC999999999999AULL, 0xBFD5555555555555ULL,
    0x3FF0000000000000ULL,
};

static double bmo_m_d(unsigned long long b) {
    return *(double *)&b;
}

static unsigned long long bmo_m_bits(double v) {
    return *(unsigned long long *)&v;
}

static int bmo_m_esinf(double v) {
    return (bmo_m_bits(v) & 0x7FFFFFFFFFFFFFFFULL) == 0x7FF0000000000000ULL;
}

/* `acc = t[0]`; `acc = acc * x + t[i]`: el orden de `horner` del oraculo. */
static double bmo_m_horner(const unsigned long long *t, int n, double x) {
    double acc = bmo_m_d(t[0]);
    int i;
    for (i = 1; i < n; i++) {
        acc = acc * x + bmo_m_d(t[i]);
    }
    return acc;
}

/* El entero mas cercano, empates a par, para |y| < 2^51. */
static double bmo_m_nearest(double y) {
    double m = bmo_m_d(BMO_M_NEAREST);
    if (y >= 0.0) {
        return (y + m) - m;
    }
    return (y - m) + m;
}

/* 2^k, para k en el rango de los normales. */
static double bmo_m_pow2(long long k) {
    return bmo_m_d((unsigned long long)(k + 1023) << 52);
}

/* Seno (lo devuelve) y coseno (en `*c`): `sin_cos_f64`. */
static double bmo_m_sincos(double x, double *c) {
    double k, r, r2, s, co;
    long long q;
    if (x != x || bmo_m_esinf(x)) {
        *c = bmo_m_d(0x7FF8000000000000ULL);
        return bmo_m_d(0x7FF8000000000000ULL);
    }
    k = bmo_m_nearest(x * bmo_m_d(BMO_M_FRAC_2_PI));
    r = (x - k * bmo_m_d(BMO_M_FRAC_PI_2_HI)) - k * bmo_m_d(BMO_M_FRAC_PI_2_LO);
    r2 = r * r;
    s = r * bmo_m_horner(bmo_m_SIN, 7, r2);
    co = bmo_m_horner(bmo_m_COS, 8, r2);
    /* El cuadrante con la saturacion de Rust, como el emisor de SPIR-V: por
     * encima de 2^63 el `cvttsd2si` de x86 da 0x8000..., y Rust satura a
     * i64::MAX (cuadrante 3). Por debajo de -2^63 los dos dan el cuadrante 0.
     * Ahi el resultado ya no significa nada; lo que importa es que sea UNO. */
    if (k >= 9223372036854775808.0) {
        q = 3;
    } else {
        q = (long long)k & 3;
    }
    if (q == 0) {
        *c = co;
        return s;
    }
    if (q == 1) {
        *c = -s;
        return co;
    }
    if (q == 2) {
        *c = -co;
        return -s;
    }
    *c = s;
    return -co;
}

double sin(double x) {
    double c;
    return bmo_m_sincos(x, &c);
}

double cos(double x) {
    double c;
    bmo_m_sincos(x, &c);
    return c;
}

double tan(double x) {
    double c;
    double s = bmo_m_sincos(x, &c);
    return s / c;
}

/* e^x: `exp_f64`. */
double exp(double x) {
    double k, r, p;
    long long ki, medio;
    if (x != x) {
        return x;
    }
    if (x > bmo_m_d(BMO_M_EXP_MAX)) {
        return bmo_m_d(0x7FF0000000000000ULL);
    }
    if (x < bmo_m_d(BMO_M_EXP_MIN)) {
        return 0.0;
    }
    k = bmo_m_nearest(x * bmo_m_d(BMO_M_INV_LN2));
    r = (x - k * bmo_m_d(BMO_M_LN2_HI)) - k * bmo_m_d(BMO_M_LN2_LO);
    p = bmo_m_horner(bmo_m_EXP, 13, r);
    ki = (long long)k;
    medio = ki / 2;
    return p * bmo_m_pow2(medio) * bmo_m_pow2(ki - medio);
}

/* ln x: `ln_f64`. x < 0 (y el NaN) da NaN; 0 da -infinito. */
double log(double x) {
    unsigned long long b;
    long long e;
    double m, s, s2, p, ef, y;
    if (x != x || x < 0.0) {
        return bmo_m_d(0x7FF8000000000000ULL);
    }
    if (x == 0.0) {
        return bmo_m_d(0xFFF0000000000000ULL);
    }
    if (bmo_m_esinf(x)) {
        return x;
    }
    b = bmo_m_bits(x);
    e = (long long)((b >> 52) & 0x7FF);
    if (e == 0) {
        y = x * bmo_m_pow2(54);
        b = bmo_m_bits(y);
        e = (long long)((b >> 52) & 0x7FF) - 54;
    }
    e = e - 1023;
    m = bmo_m_d((b & 0x000FFFFFFFFFFFFFULL) | 0x3FF0000000000000ULL);
    if (m > bmo_m_d(BMO_M_SQRT_2)) {
        m = m * 0.5;
        e = e + 1;
    }
    s = (m - 1.0) / (m + 1.0);
    s2 = s * s;
    p = bmo_m_horner(bmo_m_LN, 11, s2);
    ef = (double)e;
    return (2.0 * s * p + ef * bmo_m_d(BMO_M_LN2_LO)) + ef * bmo_m_d(BMO_M_LN2_HI);
}

/* atan: `atan_f64`. */
double atan(double x) {
    int neg, inv, red;
    double a, r;
    if (x != x) {
        return x;
    }
    neg = (int)(bmo_m_bits(x) >> 63);
    a = neg ? -x : x;
    inv = a > 1.0;
    if (inv) {
        a = 1.0 / a;
    }
    red = a > bmo_m_d(BMO_M_TAN_PI_12);
    if (red) {
        a = (a * bmo_m_d(BMO_M_SQRT_3) - 1.0) / (bmo_m_d(BMO_M_SQRT_3) + a);
    }
    r = a * bmo_m_horner(bmo_m_ATAN, 16, a * a);
    if (red) {
        r = r + bmo_m_d(BMO_M_FRAC_PI_6);
    }
    if (inv) {
        r = (bmo_m_d(BMO_M_FRAC_PI_2_HI) - r) + bmo_m_d(BMO_M_FRAC_PI_2_LO);
    }
    return neg ? -r : r;
}

/* atan2: `atan2_f64`, con los ceros con signo y los infinitos de C99. */
double atan2(double y, double x) {
    double hi = bmo_m_d(BMO_M_FRAC_PI_2_HI);
    double lo = bmo_m_d(BMO_M_FRAC_PI_2_LO);
    double pi = 2.0 * hi + 2.0 * lo;
    double pi_2 = hi + lo;
    double r, q, z;
    int yneg;
    if (x != x || y != y) {
        return x + y;
    }
    yneg = (int)(bmo_m_bits(y) >> 63);
    if (y == 0.0) {
        if (x > 0.0 || (x == 0.0 && (bmo_m_bits(x) >> 63) == 0)) {
            return y;
        }
        r = pi;
    } else if (x == 0.0) {
        r = pi_2;
    } else if (bmo_m_esinf(x)) {
        if (bmo_m_esinf(y)) {
            r = x > 0.0 ? 0.5 * pi_2 : 1.5 * pi_2;
        } else {
            r = x > 0.0 ? 0.0 : pi;
        }
    } else if (bmo_m_esinf(y)) {
        r = pi_2;
    } else {
        q = y / x;
        z = atan(q < 0.0 ? -q : q);
        if (x > 0.0) {
            r = z;
        } else {
            r = (2.0 * hi - z) + 2.0 * lo;
        }
    }
    return yneg ? -r : r;
}

/* pow: `pow_c_f64`. [!] En C `==` pesa MAS que `&`: el impar lleva sus
 * parentesis, o `(long long)y & (1 == 1)` preguntaria otra cosa. */
double pow(double x, double y) {
    double ax, ay, r;
    int entero, impar;
    if (y == 0.0 || x == 1.0) {
        return 1.0;
    }
    if (x != x || y != y) {
        return x + y;
    }
    ax = x < 0.0 ? -x : x;
    if (bmo_m_esinf(y)) {
        if (ax == 1.0) {
            return 1.0;
        }
        return ((ax < 1.0) == (y < 0.0)) ? bmo_m_d(0x7FF0000000000000ULL) : 0.0;
    }
    ay = y < 0.0 ? -y : y;
    entero = !(y > -4503599627370496.0 && y < 4503599627370496.0) || y == (double)(long long)y;
    impar = entero && ay < 9007199254740992.0 && (((long long)y & 1) == 1);
    if (x == 0.0 || bmo_m_esinf(x)) {
        r = ((x == 0.0) == (y > 0.0)) ? 0.0 : bmo_m_d(0x7FF0000000000000ULL);
        return (impar && (bmo_m_bits(x) >> 63)) ? -r : r;
    }
    if (x < 0.0 && !entero) {
        return bmo_m_d(0x7FF8000000000000ULL);
    }
    r = exp(y * log(ax));
    return (x < 0.0 && impar) ? -r : r;
}

/* Las de `float`: la de doble y un redondeo, como `math::sin` y compania. */
float sinf(float x) {
    return (float)sin((double)x);
}

float cosf(float x) {
    return (float)cos((double)x);
}

float tanf(float x) {
    return (float)tan((double)x);
}

float atanf(float x) {
    return (float)atan((double)x);
}

float atan2f(float y, float x) {
    return (float)atan2((double)y, (double)x);
}

float expf(float x) {
    return (float)exp((double)x);
}

float logf(float x) {
    return (float)log((double)x);
}

float powf(float x, float y) {
    return (float)pow((double)x, (double)y);
}

#endif /* BMO_MATH_H */
