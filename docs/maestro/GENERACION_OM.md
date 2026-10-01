# GENERACION OM -- BMO-X2, el hijo que hereda lo bueno del padre

> **OM = Orquestado Metal.** No es "un OS para PC": es un orquestador que
> juzga y reparte el metal. BMO-X es la primera generacion; BMO-X2 es la
> segunda, y nace cuando el padre haya cumplido su prueba de fuego.

Escrito el **2026-10-01**, de una conversacion con el propietario, antes de
una sola linea de BMO-X2. Lo que dijo, en sus palabras:

> *"titan++ es el inicio para la generacion de Bmo-X que es Generacion OM y no
> PC ... la razon es que usan Rust y no tengo control nada de rust ... para
> tener control total de x86_64 ... que herede todo cuando termine con bmo-X"*

> *"mejor me quedo con inti que trabaja para emitir de forma precisa y titan++
> simplemente se enfoca en app pesados y fuertes"*

---

## 1. La decision (01-10)

```text
   BMO-X2 (el kernel, los drivers, el metal)   en INTI   -- emite preciso
   lo de encima (apps pesadas, juegos, F1)     en TITAN++
   BMO-X (el padre)                            se queda en Rust, como REFERENCIA
```

Los DOS lenguajes son propios: el control de la maquina deja de depender de
rustc y LLVM. Es lo que se llama un sistema **autoalojado** (Unix con C,
Oberon, Smalltalk): escrito en su lenguaje y compilado por su compilador.

Se descarto escribir el kernel en TITAN++ (opcion B): su borrow checker
(modelo 2, sin referencias guardadas, `TITAN_MAESTRO` 6.4) no sirve para un
kernel, que vive de referencias (tablas de paginas, colas, listas), de
ensamblador y de interrupciones. Pediria un modo "metal" con reglas propias:
otro proyecto encima del lenguaje. INTI ya es "el C de BMO-X" y ya existe.

Y no contradice `TITAN_MAESTRO` seccion 10 ("TITAN++ NO escribe el kernel"):
la confirma.

## 2. "Velocidad de ASM" en TITAN++: lo que es verdad y lo que no

Un lenguaje compilado se acerca a C cuando su emisor es bueno. Llegar al
nivel de ASM escrito a mano depende de tres cosas que hoy NO estan:

```text
   el emisor          optimizacion: registros, quitar copias, desenrollar
   el silicio entero  AVX2 y FMA estan en el 5600X SIN usar (el informe lo
                      dice: "13 usadas de 36"); las tablas de FORTRAN (2b.2)
                      son lo que deja vectorizar
   la puerta a INTI   lo caliente de verdad se escribe en INTI y TITAN++ lo
                      llama (compilacion separada, `.bo` + bmo-enlazar)
```

Y la 3060 (`gpu fn`, T5) es donde TITAN++ gana de verdad en lo pesado.

## 3. El orden (sin fechas, LEY 24)

```text
   G0  BMO-X cumple su prueba de fuego: Cyberpunk 2077 corriendo. Ahi se
       sabe que funciono y que fue PARCHE
   G1  TITAN++ T1..T6 sobre BMO-X (el compilador, al principio, en Rust)
   G2  autoalojarse: el compilador de INTI y el de TITAN++ escritos en si
       mismos. Desde aqui, ni rustc ni LLVM
   G3  BMO-X2: el kernel en INTI, con los DOS JUECES desde el dia 1 (el
       certificado en la puerta de carga, J2) y sin los parches del padre
   G4  el padre como REFERENCIA: los mismos programas y bancos en los dos;
       si dan distinto, uno tiene un fallo. Se hereda CON PRUEBA
   G5  otras arquitecturas: un emisor por arquitectura (ARM, RISC-V)
```

[!] El riesgo es empezar G3 antes de G0: dos proyectos enormes para una
persona se comen el uno al otro. Y el "efecto del segundo sistema" (Brooks):
el segundo sistema quiere meterlo todo. El antidoto ya existe en la casa: las
LEYES, los guardianes y "una pieza entra solo si quita una confusion".

## 4. LA HERENCIA: lo que pasa al hijo

Se ha ganado en el metal; pasa tal cual (en INTI):

- Las LEYES y los guardianes (y sus bancos).
- BEF2 (el `.bex`), la firma y la puerta de carga.
- Las capabilities y la burocracia de las puertas (INVOKE + WAIT).
- El orquestador: declarar, juzgar, conceder; CABINA y el informe maestro.
- Los dos jueces (`TITAN_MAESTRO` 6b) y el certificado.
- El driver de la 3060 (GSP, canales, el juez de SASS) y la IOMMU.
- PROTON-X y su casa (los .exe de Windows), con sus .exe de prueba.

## 5. LO QUE EL HIJO HACE DISTINTO (y por que)

Cada vez que en BMO-X se choque con un limite de ESQUEMA (no un fallo), se
apunta aqui, con la fecha y lo que lo destapo.

| fecha | el limite en el padre | que lo destapo | lo que el hijo hace |
|---|---|---|---|
| 30-09 | 8 bloques por proceso, de 64 MiB como mucho (`obj/memory.rs`) | Cyberpunk: 26 DLL imprescindibles (~52 bloques) y libxess con 70 MiB de datos | la **declaracion de imagen** desde el dia 1: la app declara sus partes, el orquestador juzga una vez |
| 30-09 | el cargador de PE nacio para `.exe` de KiB | el `.exe` de Cyberpunk: 57 MiB | cargar por secciones, del disco a su sitio, desde el principio |
| -- | un solo nucleo al arrancar (`smp all` aparte) | el informe: "smp: solo el BSP" | los nucleos son del orquestador desde el arranque |
| -- | el certificado de TITAN++ (J2) llegara tarde, encima | `TITAN_MAESTRO` 6b.5 | la puerta de carga lo compara desde el dia 1 |
