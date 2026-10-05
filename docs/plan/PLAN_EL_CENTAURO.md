# PLAN EL CENTAURO -- TITAN++ nivel 11: `gpu fn` a la 3060

> Abierto el **2026-10-04**, con los niveles 0 a 10 de TITAN++ hechos en el
> anfitrion (amarillo: corren en el emulador, no se han visto en el Ryzen).
> El propietario, sobre las tres decisiones: *"el 1 es buena asi que vamos
> mejorando, el 2 ese mismo regla, y el 3 vamos a ver si eso es posible pero
> primero organizar ... empezar desde inicio como siempre"*.
>
> **Esto es el orden y el estudio, nada mas.** Ni una linea del nivel 11 esta
> escrita. El porque de TITAN++ en la GPU esta en
> [`TITAN_MAESTRO.md`](../maestro/TITAN_MAESTRO.md) secciones 7.4 y 8; la
> gramatica de los niveles hechos, en
> [`GRAMATICA.md`](../../toolchain/lang/titan/GRAMATICA.md).

---

# 0. LA IDEA EN UN DIBUJO

```text
   TITAN++  gpu fn suma(a: f32, b: f32) -> f32     se escribe para UNA celda
            let c = suma(xs, ys)                   xs, ys: [f32; 1024] -> 1024 hilos
              |
              |  el frontend y su juez NO saben de maquinas (7.4): comprueban
              |  la gpu fn como cualquier fn, y la regla del f32 (D2)
              v
   FORMATO  SPIR-V de computo (GLCompute)          lo que viaja: un formato,
              |                                    no un cerebro compartido
              v
   SU JUEZ  bmo-spirv-front: validate_stage        el de la casa, entero
            bmo-spirv-sm86: check                  el subconjunto que la 3060 traga
              |
              +--> HOY:    el interprete (el oraculo) da los resultados AL COMPILAR,
              |            como todo valor de TITAN++ (E0)
              +--> LUEGO:  SASS en la 3060, cuando exista LANZAR computo (Ring 0)
```

---

# 1. LAS DECISIONES

## D1 -- funciones ELEMENTALES (como FORTRAN) -- TOMADA el 04-10

Una `gpu fn` se escribe para UNA celda, y aplicada a tablas de `n` celdas es
un trabajo de `n` hilos (TITAN_MAESTRO 2b.2 y 8). Sin `gpu.launch` ni `wait`
todavia: el lanzamiento asincrono del boceto 14.12 llega despues, y con el el
estado del prestamo a la 3060 (U1). *"vamos mejorando"*: primero lo simple.

## D2 -- el f32 vive en la GPU -- TOMADA el 04-10

La regla de la casa sigue: **en la CPU, siempre `dec`**. Dentro de una
`gpu fn` se cuenta en `f32` (la 3060 cuenta asi, y redondear es parte de un
modelo de IA). Fuera de una `gpu fn`, un valor `f32` solo se GUARDA o se PASA
a otra `gpu fn`: ni se suma, ni se compara, ni se imprime. Para usarlo en la
CPU se pasa a `dec` a la vista: `round(x, 2)` -- el redondeo escrito de COBOL.

## D3 -- el permiso del `Titan.toml` (U2) -- ESTUDIADA: ES POSIBLE

Lo que se pidio: que sin `gpu = "compute"` en el `Titan.toml` una `gpu fn` no
compile. Se miro que hay, pieza a pieza:

| pieza | existe | donde |
|---|---|---|
| leer el `Titan.toml`, con la MISMA gramatica que F1 | si | `platform/shared/titan-lector/src/manifest.rs` (`parse`, pura, sin monton) |
| la puerta de la 3060 en el contrato | si | `Door::Gpu` y `Permission::Gpu` en `platform/shared/titan-contrato` |
| el `.bex` dice que pide | si, vacio | `toolchain/lang/titan/src/manifest.rs` escribe `[permissions]` sin nada |
| el juez del kernel en el PC compara lo pedido con lo usado | si | `titan juez X.bex --concede gpu` (`Unasked` / `Ungranted`) |
| el paquete encuentra su `Titan.toml` | casi | `paquete.rs` ya lee por ruta desde el paquete: es `read("Titan.toml")` |
| F1 ya niega `use gpu` sin permiso | si | `package.rs` de titan-lector (`NoPermission`) |

**Veredicto: si, y de punta a punta en el anfitrion.** Las capas lo dejan
(`toolchain` puede usar `platform/shared`, y el frontend ya usa el contrato).
Un fichero suelto no tiene `Titan.toml`, asi que no puede usar `gpu`: una app
de la 3060 es un PAQUETE. Lo que NO es posible hoy no es el permiso: es
CORRER en la 3060 (seccion 2).

## D4 -- como ENTRA un valor a `f32` -- ABIERTA (la propone G1)

Un `1.5` de TITAN++ es un `dec` exacto, y pasar a `f32` redondea (0.1 no cabe
en base 2). La propuesta: entra por un tipo DECLARADO, `let xs: [f32; 4] =
[1.0, 2.0, 2.5, 4.0]` -- la declaracion es lo que dice el redondeo, como
`dec(7, 2)` dice las cifras. Sin declarar, nada se vuelve `f32` solo.

## D5 -- quien ESCRIBE el SPIR-V -- ABIERTA (la propone G2)

`toolchain/lang/spirv/README.md` dice: *"nadie escribe SPIR-V; BMO-X lo
recibe"*. TITAN_MAESTRO 8 ya decidio que una `gpu fn` baja a SPIR-V, asi que
TITAN++ seria el PRIMER escritor de la casa. La propuesta: un crate propio,
`toolchain/lang/titan/emisor-spirv`, al lado de `emisor-x86_64` -- el frontend
sigue sin nombrar maquinas ni formatos, y el SPIR-V que sale lo juzga el juez
de spirv sin saber quien lo escribio. El README de spirv se corrige el dia
que entre.

---

# 2. LO QUE YA EXISTE Y LO QUE FALTA

```text
   EXISTE   el lector, el validador (validate_stage GLCompute) y el interprete
            (dispatch sobre buffers): toolchain/lang/spirv
            el chequeo del subconjunto de la 3060: bmo-spirv-sm86 (check)
            SPIR-V -> SASS con su juez: PLAN_LA_LENGUA_DE_LA_3060, E1-E5 [x]
            el emisor x86-64 de SPIR-V, bit a bit con el oraculo: bmo-spirv-x86-64
   FALTA    quien escriba SPIR-V desde TITAN++ (D5, G2)
            LANZAR computo en la 3060: la QMD y el banco constante 0
            (platform/drivers/gpu/ga10x/COMO_LE_HABLA_NVIDIA.md, 3d) -- Ring 0,
            DEL PROPIETARIO. Hasta entonces una gpu fn se compila, se juzga y
            se calcula con el oraculo, pero no corre en la 3060
```

---

# 3. LAS CASILLAS

## [x] G0 -- EL PERMISO (D3), antes que nada -- HECHO el 04-10

- [x] `paquete.rs` lee el `Titan.toml` del paquete con `titan_lector::manifest::parse` (solo junto a `src/`, como F1); uno que no se lee es T0089
- [x] `use gpu` / `use director` sin su permiso en `[permissions]`: T0088 (los mismos pares que F1)
- [x] el `.bex` copia en `[permissions]` lo que pide el `Titan.toml` (test `a_package_carries_the_permissions_its_titan_toml_asks_for` del emisor)
- [x] la ley L25 en `toolchain/tools/titan-leyes/LEYES.txt`
- Lo que pide una `gpu fn` para existir (el certificado con `Door::Gpu`, y `titan juez` con y sin `--concede gpu`) pasa a G1.

## [x] G1 -- EL FRONTEND: `gpu fn` y el f32 (D1, D2, D4) -- HECHO el 04-10

- [x] `gpu fn` en el parser (la palabra 25), y `f32` como tipo del nivel 11 (`f64` sigue sin sitio: T0040)
- [x] la regla D2: fuera de una `gpu fn`, un `f32` solo se guarda o se pasa; `round(x, n)` lo vuelve `dec` (T0091, `calc.rs`)
- [x] dentro de una `gpu fn`: f32 y bool, sin `print`, tablas, textos, llamadas ni bucles (T0090, `src/gpu.rs`); los numeros escritos alli son f32
- [x] D4: un numero entra a f32 solo por un tipo declarado (`let xs: [f32; n] = ...`)
- [x] la aplicacion ELEMENTAL: `mezcla(xs, ys)` con tablas de igual largo da una tabla
- [x] sin permiso `gpu`, una `gpu fn` no compila (como `use gpu`, T0088)
- [x] el certificado nombra `Door::Gpu` en la linea de cada llamada a una `gpu fn`
- [x] `titan juez` lo compara: con `gpu` concedido, de acuerdo; sin el, `Ungranted` (test `a_gpu_call_is_named_in_the_certificate_and_judged` del emisor)
- [x] los codigos nuevos, cada uno con su programa roto en `ejemplos/nivel11/`; las leyes L26 y L27
- Hoy cada hilo lo corre el calculo, en la CPU, con f32 IEEE de precision simple (cada operacion redondeada una vez); G2 y G3 lo llevan a SPIR-V y a su oraculo.

## [ ] G2 -- EL FORMATO: TITAN++ escribe SPIR-V (D5)

- [ ] `emisor-spirv`: una `gpu fn` -> un modulo GLCompute (buffers de entrada y salida, `GlobalInvocationId`)
- [ ] el SPIR-V que sale pasa `validate_stage(GLCompute)` y `bmo-spirv-sm86::check` -- si no, es un fallo del ESCRITOR, nunca del programa
- [ ] el README de spirv dice quien escribe ahora

## [ ] G3 -- LOS RESULTADOS: el oraculo al compilar

- [ ] el calculo corre cada aplicacion elemental en el interprete de spirv y guarda las celdas
- [ ] el banco `nivel11` (paquetes con su `Titan.toml`) corre en el emulador y compara su salida
- [ ] el metro con dos programas del nivel 11

## [ ] G4 -- EN LA 3060 (Ring 0, del propietario)

- [ ] LANZAR computo: la QMD y el banco constante 0 (ga10x)
- [ ] el mismo programa del banco, con sus celdas calculadas por la 3060 y comparadas con el oraculo

## [ ] G5 -- LOS PAPELES

- [ ] GRAMATICA nivel 11 y sus codigos; TITAN_MAESTRO, la entrada del nivel 11 y 14.14 al dia

---

# 4. LO QUE ESTE PLAN NO ES

- **No es el lanzamiento asincrono** (`gpu.launch`, `wait`) ni el prestamo de
  un bufer a la 3060 (U1): llegan despues de D1, con el juez del prestamo.
- **No es la IA personal**: es el escalon que ella necesita
  ([`PLAN_EL_ASISTENTE.md`](en_pausa/PLAN_EL_ASISTENTE.md) espera a este).
- **No toca Ring 0**: G4 es del propietario, y este plan solo dice que pide.
