//! **LA SONDA DE QUAKE** (L2 de `PLAN_LA_LUDOTECA`, 2026-09-25): lo que el
//! `mathlib.c` y el renderizador de software de Quake le piden a la coma
//! flotante de BMO C, caso a caso, EJECUTADO. No para en el primero que
//! falla: los cuenta, y dice cual y por que. Es el censo de lo que falta,
//! como `census` lo fue para DOOM.
//!
//! Los numeros: exactos en binario siempre que se pueda (0.5, 0.25, 1.5), y
//! el resultado sale como entero escalado -- un test de coma flotante que
//! depende de como imprime `printf("%f")` mide `printf`, no la coma flotante.

use super::*;

/// `(nombre, programa, lo que tiene que imprimir)`.
const CASOS: &[(&str, &str, &str)] = &[
    ("vec3_t y DotProduct", r#"
typedef float vec3_t[3];
#define DotProduct(x,y) (x[0]*y[0]+x[1]*y[1]+x[2]*y[2])
int main() { vec3_t a = {1.5f, 2.0f, 0.5f}; vec3_t b = {2.0f, 0.25f, 4.0f};
  printf("%d\n", (int)(DotProduct(a, b) * 100)); return 0; }"#, "550"),
    ("vec3_t por parametro (CrossProduct)", r#"
typedef float vec3_t[3];
void Cross(vec3_t v1, vec3_t v2, vec3_t c) {
  c[0] = v1[1]*v2[2] - v1[2]*v2[1]; c[1] = v1[2]*v2[0] - v1[0]*v2[2]; c[2] = v1[0]*v2[1] - v1[1]*v2[0]; }
int main() { vec3_t x = {1,0,0}; vec3_t y = {0,1,0}; vec3_t z; Cross(x, y, z);
  printf("%d %d %d\n", (int)z[0], (int)z[1], (int)z[2]); return 0; }"#, "0 0 1"),
    ("float global", r#"
float escala = 1.5f;
int main() { escala = escala * 2; printf("%d\n", (int)(escala * 10)); return 0; }"#, "30"),
    ("vec3_t global", r#"
typedef float vec3_t[3];
vec3_t vec3_origin = {0, 0, 0};
vec3_t luz = {0.5f, 0.25f, 1.0f};
int main() { printf("%d %d\n", (int)(luz[0]*100 + luz[1]*100), (int)vec3_origin[2]); return 0; }"#, "75 0"),
    ("struct con floats", r#"
typedef float vec3_t[3];
typedef struct { vec3_t origin; float yaw; int flags; } ent_t;
int main() { ent_t e; e.origin[0] = 2.5f; e.yaw = 0.5f; e.flags = 3;
  printf("%d %d %d\n", (int)(e.origin[0] * 10), (int)(e.yaw * 10), e.flags); return 0; }"#, "25 5 3"),
    ("puntero a struct con floats", r#"
typedef struct { float x, y; } p_t;
void doble(p_t *p) { p->x *= 2; p->y += 0.5f; }
int main() { p_t p; p.x = 1.25f; p.y = 1.0f; doble(&p);
  printf("%d %d\n", (int)(p.x * 100), (int)(p.y * 100)); return 0; }"#, "250 150"),
    ("float devuelto", r#"
float mitad(float x) { return x * 0.5f; }
int main() { printf("%d\n", (int)(mitad(5.0f) * 10)); return 0; }"#, "25"),
    ("muchos float por parametro (mas de 8)", r#"
float suma(float a, float b, float c, float d, float e, float f, float g, float h, float i, float j) {
  return a+b+c+d+e+f+g+h+i+j; }
int main() { printf("%d\n", (int)suma(1,2,3,4,5,6,7,8,9,10)); return 0; }"#, "55"),
    ("float e int mezclados en la llamada", r#"
float mezcla(int n, float x, int m, float y) { return n * x + m * y; }
int main() { printf("%d\n", (int)(mezcla(2, 1.5f, 3, 0.5f) * 10)); return 0; }"#, "45"),
    ("comparar floats y el ternario", r#"
int main() { float a = 0.5f, b = 0.25f; float m = a > b ? a : b;
  int n = 0; while (a > 0.1f) { a *= 0.5f; n++; }
  printf("%d %d\n", (int)(m * 100), n); return 0; }"#, "50 3"),
    ("de float a int, negativo (trunca hacia cero)", r#"
int main() { float f = -2.75f; int i = (int)f; unsigned u = (unsigned)3.9f;
  printf("%d %u\n", i, u); return 0; }"#, "-2 3"),
    ("de double a float y vuelta", r#"
int main() { double d = 0.1; float f = (float)d; double e = f;
  printf("%d\n", e != d); return 0; }"#, "1"),
    ("float por varargs (%f de printf)", r#"
int main() { float f = 1.5f; printf("%.2f\n", f); return 0; }"#, "1.50"),
    ("array de floats y aritmetica de punteros", r#"
int main() { float t[4] = {0.5f, 1.5f, 2.5f, 3.5f}; float *p = t + 1; float s = 0;
  int i; for (i = 0; i < 3; i++) s += p[i];
  printf("%d\n", (int)(s * 10)); return 0; }"#, "75"),
    ("negar y asignaciones compuestas", r#"
typedef float vec3_t[3];
int main() { vec3_t v = {1.0f, -2.0f, 0.5f}; int i;
  for (i = 0; i < 3; i++) { v[i] = -v[i]; v[i] *= 2; v[i] -= 0.5f; }
  printf("%d %d %d\n", (int)(v[0]*10), (int)(v[1]*10), (int)(v[2]*10)); return 0; }"#, "-25 35 -15"),
    ("math.h: sqrt, floor, ceil, trunc", r#"
#include <math.h>
int main() { printf("%d %d %d %d %d %d\n", (int)(sqrt(2.25) * 100), (int)floor(-1.5), (int)ceil(-1.5),
  (int)floor(2.0), (int)ceil(2.25), (int)trunc(-2.75)); return 0; }"#, "150 -2 -1 2 3 -2"),
    ("math.h: la longitud de un vector (VectorLength)", r#"
#include <math.h>
typedef float vec3_t[3];
float Length(vec3_t v) { return sqrt(v[0]*v[0] + v[1]*v[1] + v[2]*v[2]); }
int main() { vec3_t v = {3, 4, 12}; printf("%d\n", (int)Length(v)); return 0; }"#, "13"),
    ("math.h: el signo del cero de trunc", r#"
#include <math.h>
int main() { double z = trunc(-0.5); printf("%d\n", 1.0 / z < 0); return 0; }"#, "1"),
];

/// Corre un caso sin dejar que un fallo tumbe a los demas: en su propio
/// hilo (con pila de sobra: un bucle que no acaba no debe llevarse la sonda),
/// y con el preprocesador delante, como la linea de ordenes.
fn probar(src: &'static str) -> Result<String, String> {
    let h = std::thread::Builder::new().stack_size(256 << 20).spawn(move || {
        let bef = compile_with_preprocessor(src, std::path::Path::new("quake.c"), CStandard::C11).map_err(|e| format!("NO COMPILA: {e:?}"))?;
        Ok(ejecutar_bef(&bef))
    });
    match h.map(|h| h.join()) {
        Ok(Ok(r)) => r,
        _ => Err("SE CAYO AL EJECUTAR".to_string()),
    }
}

/// Un puntero a funcion que devuelve un flotante: hasta que su tipo lleve el
/// retorno, NO compila -- y lo dice. Antes compilaba y la llamada daba 0.
#[test]
fn un_puntero_a_funcion_que_devuelve_float_dice_no() {
    let src = "float cuad(float x) { return x * x; }\nint main() { float (*f)(float) = cuad; return (int)f(1.5f); }";
    let e = compile_with_preprocessor(src, std::path::Path::new("q.c"), CStandard::C11).unwrap_err();
    assert!(format!("{e:?}").contains("registro equivocado"), "{e:?}");
}

/// Lo que la sonda sabe que TODAVIA no sale, con su motivo. Todo lo demas
/// tiene que salir: un caso que se cae despues de haber salido es un fallo.
const PENDIENTES: &[&str] = &[
    // `printf` se compila formato a formato; `%f` pide convertir un double a
    // decimal en la ejecucion, y eso aun no esta.
    "float por varargs (%f de printf)",
];

/// ** El censo: imprime cada caso y cuantos salen (`--nocapture`), y FALLA si
/// uno que no esta en [`PENDIENTES`] no sale.
#[test]
fn sonda_de_quake() {
    let mut bien = 0;
    let mut caidos = std::vec::Vec::new();
    for (nombre, src, esperado) in CASOS {
        let r = probar(src);
        let ok = matches!(&r, Ok(s) if s.trim() == *esperado);
        bien += ok as usize;
        println!("[{}] {nombre}: {}", if ok { "SI" } else { "NO" }, match &r {
            Ok(s) => format!("salio `{}`, tenia que salir `{esperado}`", s.trim()),
            Err(e) => e.chars().take(160).collect(),
        });
        if !ok && !PENDIENTES.contains(nombre) {
            caidos.push(*nombre);
        }
    }
    println!("sonda de quake: {bien} de {} casos", CASOS.len());
    assert!(caidos.is_empty(), "se cayeron casos que ya salian: {caidos:?}");
}
