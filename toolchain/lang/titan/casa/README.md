# LA CASA -- la libreria de TITAN++

Modulos que cualquier app pide en vez de copiarlos. Viajan dentro del
compilador (`src/casa.rs`, con `include_str!`): un paquete no lleva copia
ni ruta, y cada build usa la casa del compilador que lo construye.

```text
mod main "mi app"
use director
use casa.medida

fn main()
    let a = medida.ancho()
```

| Modulo | Que trae | Pide |
|---|---|---|
| [`cifras`](cifras.titan) | `es_cifra(b)`, `valor(b)`: las cifras de un texto en bytes | nada |
| [`medida`](medida.titan) | `ancho()`, `alto()`: la medida que guardo RESOLUCION | `screen` (usa `director`) |

## Las reglas

- Se pide con el nombre entero, `use casa.medida`, y se llama por el suyo,
  `medida.ancho()`. Quien lee la cabecera sabe que no es del paquete.
- Un modulo de la casa es UN fichero, y solo usa la casa o un nodo de BMO-X
  (`director`, `sonido`, `gpu`). Nunca un modulo de una app: la casa esta
  DEBAJO de todas (ley L8, las capas solo bajan).
- Lo que la casa usa de BMO-X lo pide la app en su `Titan.toml` (U2): la casa
  no concede nada.
- Una app no puede tener un modulo propio con el nombre de uno de la casa
  que pide (T0052).

## Para agregar un modulo

1. Escribirlo aqui, con su cabecera `mod nombre "que hace"`.
2. Sumarlo a `MODULES` en [`src/casa.rs`](../src/casa.rs) y a esta tabla.
3. Solo con DOS usos reales por lo menos: la casa nace de lo que ya se
   copiaba, no de lo que algun dia podria servir.

`every_house_module_compiles_alone` (en `src/paquete.rs`) compila cada uno
solo; las apps que los piden los ejercen en sus pruebas del emisor.
