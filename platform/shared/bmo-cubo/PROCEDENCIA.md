# PROCEDENCIA de `bmo-cubo`

Este crate NO se escribio aqui. Es `cubo-neutro`, del estudio de Direct3D del
propietario:

| | |
|---|---|
| repositorio | https://github.com/AndreeSalazar/EPICX-FRAMEWORK-DirectX12 |
| rama | `estudio-d3d` |
| commit | `412542e` (el crate entra en `11f4fa9`; las `Reglas`, antes de `412542e`) |
| carpeta | `cubo-neutro/` |
| sha256 del original | `lib.rs` c32ccd01..., `mat.rs` 9cf77aa5..., `num.rs` 066b4839... |
| traido | 2026-09-25 |

## Lo que cambio al traerlo, y NADA mas

1. **Los comentarios, a ASCII** (la regla de `toolchain/tools/ascii-sweep`):
   las tildes caen (`aritmetica`), `x` por el signo de multiplicar, `*` por el
   punto medio. Comprobado con un script: fuera de los comentarios, las unicas
   lineas que cambian son TRES textos de `assert!` (`lib.rs` 240 y 426,
   `num.rs` 120). Ni una cuenta.
2. **La cabecera de BMO-X** en `lib.rs` (`generacion`, `capa`, `[carril]`,
   `[cuesta]`) y `pub mod referencia;`.
3. **`src/referencia.rs` es PROPIO**: las huellas de las tres capturas de
   D3D12 en la 3060, y la prueba de que el juez las da.

## Por que se puede fiar

- Compila para `x86_64-unknown-none` (el target del kernel), comprobado aqui.
- Sus 14 pruebas pasan en Linux, y las 3 de `referencia` tambien.
- El juez corrido en Linux dio **0 pixeles distintos** de 921.600 contra las
  capturas de D3D12 en los fotogramas 0, 30 y 60: el seno, el coseno y la raiz
  son propios (`num.rs`), sin libm, y por eso no cambia de un sistema a otro.

Por que existe y lo que puede dar: `docs/plan/PLAN_EL_CUBO.md`.

## Si el original cambia

Se vuelve a traer entero, se repite el paso 1 y se comprueba que
`referencia::pruebas::el_juez_da_las_huellas_de_la_3060` sigue verde. Un
arreglo que se haga AQUI se manda tambien alla: dos copias que divergen son
dos jueces.
