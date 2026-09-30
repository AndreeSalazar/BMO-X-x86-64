# TITAN++ -- LA GRAMATICA, nivel a nivel

> El **porque** de cada decision esta en `docs/maestro/TITAN_MAESTRO.md`. Aqui
> va **lo que se escribe**, y solo lo que el frontend YA entiende: un nivel se
> escribe aqui el dia que su banco (`ejemplos/nivelN/`) pasa entero.

Decidido por el propietario el **2026-09-30**:

```text
   los bloques      por SANGRIA, como INTI (no llaves)
   el primer hola   en la CONSOLA (F12), como INTI; la ventana, despues
```

---

## Nivel 0 -- un programa que saluda (1 palabra: `fn`)

```text
# hola.titan
mod main "saluda"

fn main()
    print("hola")
```

### Las piezas

```text
   comentario    `#` hasta el final de la linea (fuera de un texto)
   cabecera      la PRIMERA linea que no es blanca ni comentario:
                    mod NOMBRE "que hace"
                 (U3: un modulo dice lo que hace; la misma regla que el
                 lector de F1, `platform/shared/titan-lector`)
   funcion       fn NOMBRE()           y debajo, SANGRADO, su cuerpo
   llamada       NOMBRE(TEXTO, ...)    una por linea
   texto         "..." en una linea; dentro valen  \"  \\  \n
   nombre        una letra y despues letras, cifras o _
```

### La sangria (la misma regla que INTI)

```text
   un nivel .............. CUATRO espacios, exactos
   un tabulador .......... T0010, sin excepciones
   un ancho que no es
     multiplo de 4, o que
     salta dos niveles, o
     vuelve a un margen
     que nadie abrio ..... T0012
```

### Lo que ya existe dentro del nivel 0

```text
   print(TEXTO, ...)      escribe los textos en la consola, uno detras de otro,
                          y salta de linea al acabar
   una fn propia          se puede definir y llamar sin argumentos
   fn main()              OBLIGATORIA: por ahi empieza el programa (T0050)
```

### Lo que el nivel 0 dice que TODAVIA NO

Una de las 25 palabras de un nivel que aun no existe no es un error de
sintaxis cualquiera: es **T0040**, y dice en que nivel llega:

```text
   let x = 1        T0040  `let` llega en el nivel 1 (calcular)
   while ...        T0040  `while` llega en el nivel 4 (repetir)
```

### Los codigos

| codigo | que |
|---|---|
| T0001 | falta la cabecera `mod nombre "que hace"` |
| T0010 | un tabulador en la sangria |
| T0012 | una sangria que no es de 4 en 4, o salta, o vuelve a donde no hubo |
| T0020 | un texto sin cerrar |
| T0021 | un caracter que no es del lenguaje |
| T0022 | una `\` en un texto que no es `\"`, `\\` ni `\n` |
| T0030 | se esperaba otra cosa en ese sitio |
| T0031 | una funcion sin cuerpo |
| T0040 | una palabra de un nivel que aun no existe |
| T0050 | no hay `fn main()` |
| T0051 | se llama a algo que no existe |
| T0052 | una funcion definida dos veces |
