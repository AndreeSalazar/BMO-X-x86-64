/* math.h -- lo que hoy se puede hacer con la ruta SSE, y nada mas.
 *
 * == Por que este fichero es tan corto ==
 *
 * BMO C evalua coma flotante por SSE y ya sabe PASAR y DEVOLVER `double`, asi
 * que las funciones que son **una comparacion y un signo** salen solas. Las
 * demas --`sqrt`, `sin`, `cos`, `atan`, `pow`, `log`-- no son eso: son series
 * o instrucciones dedicadas, y escribirlas a medias daria numeros que parecen
 * bien y no lo estan.
 *
 * No estan, y se dice. Un `sin` que devolviera el argumento seria peor que un
 * error de compilacion: el programa correria y pintaria el mundo torcido.
 * (`sqrt`, `trunc`, `floor` y `ceil` SI estan: ver abajo, lo exacto.)
 *
 * [!] `sqrtsd` es UNA instruccion de SSE2 y entraria por `intrinsics.toml` sin
 * escribir una serie -- ese es el camino cuando haga falta, y es una fila de
 * tabla, no un fichero de C.
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

#endif /* BMO_MATH_H */
