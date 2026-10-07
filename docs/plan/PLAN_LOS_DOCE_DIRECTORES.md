# PLAN LOS DOCE DIRECTORES -- PROTON-X aprende a usar TODO el Ryzen

> Abierto el **2026-10-07**, tras la segunda corrida de Cyberpunk 2077 en el
> metal con el monton que crece (06-10 22:43). El propietario: *"creo que va
> a ser largo pero vamos a aplicar por completo, entonces nuevos planes, es
> simplemente educar mas"*, y sobre los nucleos: *"si necesita todos los
> hilos y core en mi CPU TODOS para eso, no? [...] que cada uno son
> directores inteligentes por motivos para administrar y cooperando y
> independiente e dependiente asi"*.
>
> Lo de antes, que este plan NO repite: las corridas y la pila de DX12 en
> [`PLAN_LAS_TRES_GRANDES.md`](PLAN_LAS_TRES_GRANDES.md) (seccion 6 y 7.1);
> las casillas del kernel de varios nucleos, H0 a H3, en su seccion 4; el
> modelo maestro y obreros en
> [`SMP_MAESTRO.md`](../maestro/SMP_MAESTRO.md) seccion 4. Este plan dice
> QUE APRENDIO el metal, el MODELO de los directores y EL ORDEN; las
> casillas que ya viven en otro plan se nombran, no se copian.

---

# 0. LO QUE DIJO EL METAL EL 06-10, DICHO CLARO

```text
   que                      el numero                    que significa
   el monton de la casa     87 MiB en uso, crecio 64     el arreglo del 07-10
                                                         funciono: paso de 17 s
                                                         a 95 s
   D3D12                    1065 PSO, 1495 recursos,     monta el renderizador;
                            2 ExecuteCommandLists,       todavia no muestra un
                            0 Present                    cuadro
   el codigo nativo         "sin bloque sellado"         TODOS los sombreadores
                                                         van por el interprete
   la VA de bloques         "SIN SITIO: agoto los 512    la causa de lo de
                            MiB de VA de bloques"        arriba (seccion 1)
   el choque                t=95 s, salto a 0x10024b640  la consecuencia de lo
                            (FUERA de la imagen)         de arriba (seccion 1)
   el sonido                96 tirones, el peor de       el hilo del sonido no
                            33,5 s                       recibe su turno
                                                         (seccion 2)
   los nucleos              "smp: solo el BSP"           TODO el juego en UN
                                                         hilo del Ryzen de 12
```

---

# 1. LA CADENA DEL CHOQUE (medida en el codigo el 07-10)

```text
   1  cada PSO nuevo        `nativo::registrar` agrega su x86-64 al codigo de
                            TODOS los anteriores y pide UN BLOQUE NUEVO con
                            todo, sellado; el viejo se suelta
   2  el kernel no reusa    la VA de un bloque soltado NO vuelve
      la direccion          (`memory.rs`: "el cursor no vuelve", a proposito:
                            un handle viejo no puede volver a resolver)
   3  gasto cuadratico      100 PSO con 2 MiB de codigo de media = 200 MiB de
                            VA; los 512 MiB se acaban a mitad de la carga
   4  el sello falla        "sin bloque sellado": se queda el bloque VIEJO,
                            pero el PSO nuevo YA esta en la lista de los
                            traducidos, con su sitio en el codigo NUEVO
   5  el salto              el primer Draw de ese PSO llama a
                            base_vieja + sitio_nuevo: mas alla del final del
                            bloque. Base cerca de 0xFFxx_xxxx + 2,4 MiB =
                            0x1_0024_b640, la direccion del choque
```

Y lo mismo, mas despacio, con los ficheros: cada `leer_fichero` (los de
menos de 1 MiB) y cada `escribir_fichero` pide un bloque nuevo, y su VA
tampoco vuelve.

---

# 2. POR QUE NO SUENA LA INTRO

El sonido SI arranca (`IAudioClient::Start`), pero el anillo es de 65472 B:
**170 ms** de musica. El hilo del sonido del juego tiene que rellenarlo cada
pocos milisegundos. Hoy:

```text
   un nucleo        todo PROTON-X corre en el BSP; los otros 11 hilos del
                    Ryzen duermen
   hilos de la      los hilos de Windows son COOPERATIVOS (`hilos.rs`): uno
   casa             corre hasta que ESPERA algo; nadie le quita el turno
   lo largo         crear un PSO (traducir DXIL) o un Draw interpretado dura
                    MUCHO y no cede: el hilo del sonido no corre
   la hora          preguntar la hora ya cede cada 64 veces (tanda52), pero
                    eso solo ayuda si el que da vueltas pregunta la hora
```

El resultado: 40141 tramas en silencio y un corte de 33,5 s.

---

# 3. EL MODELO: DOCE DIRECTORES (educar a PROTON-X)

El Ryzen 5 5600X tiene **6 nucleos y 12 hilos**. Cyberpunk esta hecho para
usarlos TODOS: un hilo principal, hilos que graban listas de dibujo, hilos
de carga, de fisica y el del sonido. En Windows, cada hilo del Ryzen es un
**director**: tiene SU cola de trabajo y decide solo (independiente), pero
todos comparten la memoria, la GPU y el sonido, y se avisan (dependientes).

```text
   el director        quien es              que decide SOLO       de quien depende
   el MAESTRO (BSP)   el nucleo 0           los drivers, la       de nadie: es el
                                            CABINA, el reloj      unico que toca el
                                            del sistema           hierro
   un DIRECTOR        cada uno de los 12    que hilo corre en     del maestro para
                      hilos del Ryzen       SU nucleo ahora, y    los drivers; de
                                            cuando cederlo        los demas para
                                            (su propia cola)      robar trabajo si
                                                                  su cola se vacia
   el director del    un hilo con           corre ANTES que       del juego, que
   SONIDO             prioridad (MMCSS      nadie cuando su       rellena el anillo
                      "Pro Audio")          evento toca
   los OBREROS de     tareas cerradas de    la franja de la       de la trama que
   la trama           la casa (Ring 3)      imagen que les toca   las junta
```

**Las tres reglas** (las de Cell de `SMP_MAESTRO.md`, dichas para un juego):

1. **Independiente:** cada director tiene SU cola y SU reloj (el LAPIC de su
   nucleo). Decidir quien corre no pide permiso a nadie.
2. **Cooperando:** un director sin trabajo ROBA del que tiene de mas; un
   director que despierta a un hilo de otro le manda una IPI ("replanifica").
3. **Dependiente, pero aislado:** los hilos de UNA app comparten su espacio
   de direcciones y nada mas; un director no ve los datos de otra app. Lo
   del kernel, cada pieza en su fichero (`task/percpu.rs`, `plat/smp/`).

**SMT no son seis nucleos mas** (`SMP_MAESTRO.md`): dos hilos del mismo
nucleo comparten L1 y L2. Los hilos que CALCULAN van primero uno por nucleo;
los hermanos SMT, para los que ESPERAN (carga, sonido, red).

**Inteligentes SEGUN LA CPU, no con un doce escrito.** El propietario
(07-10): *"que ese core y hilo esten basado que SEAN inteligente es
DEPENDIENDO de CPU para usar con todo por algo"*. Los "doce" del nombre son
los del 5600X de hoy; el reparto NO lleva un 12 dentro. Cada director sale
de lo que la CPU DICE de si misma al arrancar:

```text
   lo que se lee            de donde                       para que
   cuantos nucleos y        CPUID (hoja 0x8000_001E en     cuantos directores
   cuantos hilos por        AMD, 0x0B/0x1F en Intel) y      hay, y quien es
   nucleo                   el MADT de ACPI                 hermano SMT de quien
   que nucleos comparten    CPUID 0x8000_001D (la cache)   a quien se roba
   L3 (el CCX/CCD)                                         primero (el mas
                                                           cercano)
   si los nucleos son       CPUID 0x1A (hibrido de         el sonido y el hilo
   iguales (o P y E)        Intel)                         principal, a los
                                                           grandes
   AVX2, AVX-512, XSAVE     CPUID 1, 7 y 0x0D              cuanto guarda cada
                                                           cambio de hilo
```

Y lo leido se escribe como UNA FILA de la tabla de `SMP_MAESTRO.md` seccion
6 (*"tablas, no cerebros"*): un Zen 2, un Zen 3 o un Intel hibrido son tres
filas y salen tres repartos distintos, sin un `if` por modelo en el
kernel. `cpu_vendor/ryzen_5_5600x/` es la fila de HOY, no la unica.

---

# 4. LAS FASES Y SUS CASILLAS

## 4V. LAS DIRECCIONES (sin Ring 0 salvo V4)

- [x] **V1 -- el choque de los 95 s.** Un traducido cuyo codigo no esta en
      el bloque sellado vivo NO se llama: va por el interprete. **Como se
      sabe:** una prueba en el banco con un `sellar_codigo` que dice que no a
      partir del segundo PSO: el segundo dibuja IGUAL (por el interprete) y
      nada salta fuera del bloque.
      07-10, HECHO: `proton-x-casa/src/nativo.rs` (`cabe`; la prueba
      `un_pso_que_no_entro_en_el_bloque_no_se_llama`, y su NO: sin `cabe`,
      el sitio del nuevo cae fuera del bloque vivo).
- [x] **V2 -- sellar cuando hace falta, no en cada PSO.** `registrar` solo
      agrega el codigo; el bloque se rehace la primera vez que un dibujo o un
      Dispatch necesita codigo que aun no esta sellado. Una carga de 1065 PSO
      sella UNA vez. **Como se sabe:** la prueba cuenta los sellados: 3 PSO y
      un Draw = 1 sello.
      07-10, HECHO: `Estado::al_dia` (`mil_pso_sellan_una_vez`: 3 PSO y 10
      dibujos, 1 sello; uno mas, 2 sellos y el viejo suelto). Los bancos de
      `textura.rs` y `tuberia.rs` ya no oyen el aviso al crear el PSO: lo
      dice el primer dibujo nativo.
- [ ] **V3 -- los ficheros por el bloque de paso.** `leer_fichero` y
      `escribir_fichero` usan el bloque de paso de A LA CARTA (1 MiB, uno y
      se queda) en vez de pedir uno por fichero. **Como se sabe:** el pulso
      dice cuantos bloques pidio PROTON-X; leer 1000 ficheros no lo sube.
      07-10: el CODIGO hecho (`proton-x/src/plataforma.rs`, `con_paso`;
      a ESTRATOS, el de paso si cabe en 1 MiB). Falta la medida, que es V5.
- [x] **V4 -- `[RING 0]` sellar por TRAMOS.** Un bloque de codigo que solo
      crece: `MEM_OP_SELLAR_HASTA(bloque, bytes)` sella las paginas nuevas y
      deja escribibles las de detras. El codigo NO se mueve nunca (ni un
      puntero viejo, ni VA gastada). **Como se sabe:** prueba del kernel:
      escribir en lo sellado falla; en lo de detras, no; con permiso del
      propietario.
      07-10, HECHO con el permiso del propietario ("si, no olvides aislar por
      completo"). AISLADO asi: la CUENTA (que paginas, solo crece, a
      paginas enteras; si una escritura del kernel toca lo sellado) es pura
      y con banco, en `bmo-imagen-juicio::sello` (2 pruebas, una que dice
      NO); el kernel solo remapea con su respuesta (`obj/memory.rs`:
      `sellado` es ahora un PREFIJO en bytes; `sellar` = `sellar_hasta`
      del bloque entero; si el remapeo falla, el bloque entero cuenta como
      sellado). `MEM_OP_SELLAR_HASTA = 0x07` y `SELLAR_FUERA = 6` en
      `bmo-abi` y `userland` (`Memoria::sellar_hasta`). La casa:
      `Plataforma::cuaderno` y `nativo::Estado::en_cuaderno` (la prueba
      `el_cuaderno_solo_crece_y_no_se_mueve`: misma direccion, lo sellado
      no se reescribe, lleno va a un bloque entero y el cuaderno no se
      suelta). PROTON-X abre el mas grande que de el kernel (64, 32, 16 u
      8 MiB). Falta VERLO en el metal: la foto dice `en el cuaderno (X de
      Y MiB)`.
- [ ] **V5 -- la cabina lo dice.** La foto del pulso de PROTON-X dice la VA
      de bloques gastada y cuantos bloques vivos (de 8). **Como se sabe:** la
      linea en el DIARIO del metal.
      07-10, la MITAD: la foto dice `# el codigo nativo: N sello(s), K KiB
      de codigo, M PSO traducidos` (y `SIN BLOQUE` si el kernel dijo que
      no; `tests/corre/de_hoy.rs`). Falta la VA de bloques: hoy el kernel no
      la publica.

## 4T. EL TURNO, mientras hay UN nucleo (sin Ring 0)

- [x] **T1 -- lo largo PRESTA el turno al sonido.** Cada orden de una lista
      de D3D12 y cada PSO creado RESPIRAN (`hilos::respirar`, como mucho
      cada 1 ms): el latido del sonido, y si el hilo del sonido ya puede
      seguir, se le PRESTA el turno y vuelve a quien lo presto
      (`Planificador::prestado`), no a la rueda. **Solo** al del sonido: un
      hilo cualquiera del juego entraria en D3D12 a mitad de la lista (eso
      es H2.1, la casa con cerrojos). **Como se sabe:** prueba en el banco
      con hilos de verdad: el principal hace algo largo sin esperar; el del
      sonido rellena a mitad; otro hilo del juego, listo, NO corre.
      07-10, HECHO: `proton-x/src/hilos.rs` (`urgente`, `prestado`,
      `del_sonido`; `pruebas_turno.rs`, 2 pruebas) y
      `proton-x-casa/tests/corre/turno.rs` (`lo_largo_presta_el_turno_al_
      sonido_y_a_nadie_mas`; su NO: 60 ms sin respirar, el sonido no corre
      ni una vez). Se respira en `d3d12::correr` (cada orden) y al crear
      cada PSO grafico y de computo. Lo que NO respira todavia: UN dibujo
      enorme por la CPU (dentro de `lote::en_cpu`), que con el codigo
      nativo de vuelta (V2) deberia durar menos que el anillo (170 ms).
- [x] **T2 -- el sonido primero.** Cuando el anillo de un `IAudioClient`
      por evento baja de la mitad, su evento se enciende y el hilo que lo
      espera pasa DELANTE en el siguiente turno (`SetThreadPriority`,
      `AvSetMmThreadCharacteristics` "Pro Audio"). **Como se sabe:** el
      banco: con un hilo que no espera nunca, el del sonido rellena a
      tiempo.
      07-10, HECHO de otra forma, mas firme: el hilo del sonido se reconoce
      por lo que ESPERA (`SetEventHandle` marca su evento: `del_sonido`), no
      por lo que dice de si (la prioridad o MMCSS, que un motor puede no
      pedir). `SetThreadPriority`/`GetThreadPriority` ya guardan y devuelven
      la de verdad (-15, -2..2, 15; lo demas, ERROR_INVALID_PARAMETER), para
      los directores. La prueba es la de T1. El pulso dice `# el turno
      prestado al sonido: N vez/veces`.
- [ ] **T3 -- medirlo en el metal.** Los tirones del audifono por debajo de
      10 en la intro. **Como se sabe:** la cabina de audio del SALIDA.TXT.

## 4H. LOS DOCE DIRECTORES `[RING 0]`, con permiso

Las casillas son las de la seccion 4 de
[`PLAN_LAS_TRES_GRANDES.md`](PLAN_LAS_TRES_GRANDES.md) (H0.1 a H3.2): no se
copian. Lo NUEVO de este plan, que alli no estaba:

- [ ] **H4.0 -- los directores salen de la CPU.** Al arrancar, el kernel
      lee la topologia de VERDAD (CPUID y MADT, la tabla de arriba) y la
      escribe como una fila de `SMP_MAESTRO`; cuantos directores, quien es
      hermano de quien y que L3 comparten salen de esa fila. **Como se
      sabe:** una prueba en el banco con las filas del 5600X (6x2, una L3),
      de un Zen 2 de dos CCX (3+3 con dos L3) y de un Intel hibrido (P y E):
      tres repartos distintos y ningun `if` por modelo; y en el metal,
      `cabina smp` dice 6 nucleos, 12 hilos, una L3.
- [ ] **H4.1 -- robar trabajo.** Un director con la cola vacia mira las de
      los demas (primero las de su mismo nucleo SMT, luego las de la L3) y
      se lleva un hilo listo. **Como se sabe:** `tandaH1.exe` con 12 hilos:
      ningun director ocioso con otro con cola.
- [ ] **H4.2 -- la prioridad del sonido entre nucleos.** El hilo de MMCSS
      "Pro Audio" se despierta con una IPI en el director que este menos
      ocupado. **Como se sabe:** el metal: tirones 0 con el juego cargando.
- [ ] **H4.3 -- los obreros de la trama.** El interprete parte un Draw en
      franjas de la imagen y las reparte entre obreros de Ring 3 de la MISMA
      app (mismos bytes que con uno). **Como se sabe:** el banco: la escena
      dura da los mismos bytes con 1 y con 6 obreros.
- [ ] **H4.4 -- la cabina de los directores.** `cabina smp` dice, por cada
      director, cuanto corrio, cuanto robo y a quien espera. **Como se
      sabe:** la linea en el SALIDA.TXT.

## 4S. LOS SOMBREADORES QUE PIDIO EL JUEGO

Viven en la pila A de la seccion 7.1 de
[`PLAN_LAS_TRES_GRANDES.md`](PLAN_LAS_TRES_GRANDES.md): 13 a 17 (ya
estaban) y 18 a 21 (nuevos del 06-10: la operacion 48, la instruccion 38, un
array de structs o vectores, un operando que no es un numero, y el bucle con
mas de una salida en un sombreador de PIXELES).

## 4P. LO QUE QUEDA DE D3D12 EN ESTA CORRIDA

- [ ] **P1 -- `CreatePlacedResource` que se solapan.** Dos recursos en el
      mismo sitio de un monton ven los MISMOS bytes. **Como se sabe:** un
      juez de Windows que escribe por uno y lee por el otro.
- [ ] **P2 -- una tabla de descriptores en un parametro mas alla del 16.**
      **Como se sabe:** un juez con una firma de 20 parametros.
- [ ] **P3 -- el primer Present de Cyberpunk.** **Como se sabe:** el DIARIO
      del metal dice `Present` y la ventana muestra algo.

---

# 5. EL ORDEN

```text
   1  V1, V2, V3     el choque y el codigo nativo de vuelta (Ring 3, ya)
   2  V5, T1, T2     verlo, y el sonido con UN nucleo (Ring 3)
   3  la pila A      los sombreadores que el juego pidio (18 a 21)
   4  V4 [RING 0]    el bloque de codigo que solo crece
   5  H0.2, H2.1     los 42 Sync de la casa, ordenados (sin encender nada)
   6  H4.0, H1, H4   los directores, segun la CPU [RING 0]
   7  P1 a P3        el primer cuadro
```

La regla de siempre: nada de la fila 6 sin la 5 hecha. Encender doce
directores con 42 sitios que suponen uno solo seria buscar fallos a ciegas.
