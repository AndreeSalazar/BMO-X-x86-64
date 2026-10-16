# PLAN EL TOQUE -- la RAM se llena al USAR, y se vacia sola lo que dejo de usarse

> El propietario (15-10): *"el usuario ABRE pero en la entrada se ve todo
> listo y tiene todo, pero en tiempo real TODOS no consume la RAM como que
> es por defecto sino que A LA HORA de que agarre cualquier ejecucion o
> tareas la RAM se llena [...] Photoshop el ejemplo es bueno porque todos
> funcionan PERO es a lo bruto [...] BMO-X guarda BASADO lo que vas a usar
> en funcionalidad pero si no se usa mucho tiempo se elimina porque ya
> cumplio y pienso que en 10 segundos lo elimina si no es mantenido. (Recorda
> que eso tiene que tener carril ESPECIFICO)"*.
>
> **Es un plan, no es para ahora**: se empieza el dia que el propietario lo
> diga. Toca el corazon de la memoria (`ring0/mm`), asi que va despacio y
> medido.

---

## 0. La respuesta corta: existe, y BMO-X todavia no lo tiene

```text
   que es        PAGINACION BAJO DEMANDA (la pagina se pide al TOCARLA, no
                 al abrir) + RECORTE DEL CONJUNTO DE TRABAJO (lo que no se
                 toca en un rato se suelta). Linux, Windows y macOS lo hacen
                 desde los setenta (Multics, VMS): no es nuevo, y funciona
   que hace hoy  BMO-X al ADMITIR un `.bex` pide y copia TODAS sus
   BMO-X         paginas (el informe del 15-10: `d.bex` 5,3 MB en el disco,
                 10,6 MB mapeados -- ceros incluidos), y una lamina de una
                 app nace entera
   lo que daria  abrir es instantaneo y barato; una app ENORME (CraftPho,
                 ILLAPA) solo ocupa lo que se esta usando; lo frio vuelve
                 a la RAM libre solo
   lo que NO     el CODIGO y lo leido del disco se pueden soltar: se leen
   se puede      otra vez. Lo que la app ESCRIBIO (un lienzo pintado, una
   soltar        nota sin guardar) NO: BMO-X no tiene "swap" (un disco donde
                 guardarlo), y soltarlo seria perderlo. Ahi decide la app
                 con el contrato de su lamina (EL3 abajo)
```

**Los 10 segundos**: para lo que se RE-LEE del disco (codigo, recursos) es
razonable, porque volver a tocarlo cuesta un fallo de pagina y una lectura
(microsegundos con la cache de disco, milisegundos sin ella). Una pagina
que se usa no caduca nunca: cada toque reinicia su reloj (el bit `A` de la
tabla de paginas lo pone la CPU sola). El numero se mide, no se adivina: va
en el PERFIL, como todo lo del Ryzen.

---

## 1. El carril propio (lo pidio el propietario)

```text
   Ultra_kernel_x86-64/kernel/src/ring0/mm/toque/
      roja.rs       el FALLO DE PAGINA que materializa: mira de quien es la
                    direccion, pide el marco, lo llena (ceros, o lo leido
                    del disco) y lo mapea. Toca tablas de paginas vivas
      amarilla.rs   el BARRIDO: cada tanto mira el bit A de las paginas
                    soltables, apunta la edad, y suelta las frias (invalida
                    el TLB). Nunca una pagina sucia sin quien la guarde
      verde.rs      la CUENTA: cuanto esta mapeado, tocado, soltado y vuelto
                    a pedir, por app, para el informe (`save`, capitulo 3)
```

Con su `[carril]`, su `[consumo]` (el barrido es `LATE`: despierta cada
tanto) y su `[riesgo]`, como todo fichero de Ring 0 (el contrato, R18 y
R21).

---

## 2. La escalera

- [ ] **EL0 -- MEDIR antes de tocar nada.** Por app: paginas mapeadas,
      paginas TOCADAS de verdad (el bit A al cerrarla) y cuanto de eso es
      codigo, ceros y laminas. **Como se sabe:** el informe lo dice para
      `d.bex`, ECCE, TINTA y MUNDO; si lo tocado es casi todo, este plan
      vale poco y se dice.
      **15-10, HECHO EN EL ANFITRION** (compila el kernel y el director, el
      contrato y la pila en verde; el kernel no corre en el anfitrion, asi
      que la cuenta en si solo la da el Ryzen): el carril `ring0/mm/toque/`
      nace con su verde, `verde.rs`, que SOLO LEE las tablas -- por region
      (imagen y pila, memoria pedida, lo demas; la pantalla no cuenta) las
      paginas que estan, las del bit A (tocadas) y las del bit D (escritas).
      Se cuenta al morir, en el enterrador, justo antes de desmontar (el
      unico momento en que nadie mas camina ese espacio), y en vivo SOLO del
      que pregunta (su espacio es el que corre): un tercero vivo no se
      camina. `INFO_PROG_TOQUE` (0xDA) lo lleva al director, y `save` pinta
      una tabla nueva en el capitulo 5, "lo que TOCO de lo que tiene", con
      la columna que importa: SIN TOCAR. Falta: verla en el Ryzen (hoja 6b
      de EL_FOCO, fila EL0).
- [ ] **EL1 -- los CEROS al tocarlos.** La region de ceros de un `.bex` (el
      `.bss`: 5 MB de los 10 de `d.bex`) se mapea NO PRESENTE y cada pagina
      nace a cero en su primer fallo. Es lo mas seguro: no hay nada que
      leer ni que perder. **Como se sabe:** el emulador cuenta los fallos y
      el programa da la misma salida; el informe, los MB que no se pidieron.
- [ ] **EL2 -- el CODIGO al tocarlo**, leido del `.bex` (de la cache de
      disco). Pega con B1 (la lente lee el codigo entero al admitir): se
      verifica UNA vez, se guarda su huella, y despues las paginas pueden
      irse y volver; al volver se comprueba su BLAKE3 contra la huella.
- [ ] **EL3 -- el contrato de la lamina dice si se puede soltar.** Una
      clase nueva en el censo (A1): `RECREABLE` -- la app sabe rehacerla
      (una miniatura, una cache) -- frente a las de siempre, que se quedan.
      Solo las RECREABLES se sueltan, y la app recibe el aviso.
- [ ] **EL4 -- el BARRIDO de lo frio** (`toque/amarilla.rs`): la edad por
      el bit A, el tope del PERFIL (10 s de salida), y soltar solo codigo,
      ceros sin tocar y laminas RECREABLES. **Como se sabe:** una app
      quieta 10 s baja su RAM en el informe; al tocarla vuelve igual, y el
      tiempo del primer toque se mide.
- [ ] **EL5 -- EN GLOBAL**: el director, los servicios y PROTON-X (que ya
      lo pide: "falta paginacion bajo demanda", PLAN_PROTON_X y LUDOTECA).

- [ ] **EL6 -- EL PUNTERO AVISA** (el propietario, 15-10: *"el puntero
      cuando apunte es potencial indicador que es 50/50 posibilidad y si le
      da es 100% y el sistema actua"*). Lo que no esta en RAM vive en el
      disco (ESTRATOS) y se trae ANTES de que haga falta: el raton QUIETO
      sobre un icono o un boton (~100 ms) pide sus paginas al disco en
      segundo plano; el clic, si llega, las encuentra ya en la RAM; si no
      llega, se sueltan con el barrido de EL4. Es lo que hacen los
      navegadores con un enlace (precarga al pasar por encima) y Windows con
      SuperFetch. **Como se sabe:** el tiempo de clic-a-ventana con y sin
      el aviso, en el Ryzen, y cuantas precargas se tiraron sin clic.

### Por que sale a cuenta (las cifras, de memoria; se miden con `disco banda`)

```text
                       primera lectura    lo que da por segundo
   disco duro (HDD)    ~5-10 ms           ~150 MB/s
   SSD SATA (el del    ~0,1 ms            ~550 MB/s     <- el del Ryzen hoy
   Ryzen: "SATA Gen3")                                     (`save`, capitulo 6)
   NVMe Gen3 / Gen4    ~0,02 ms           ~3,5 / ~7 GB/s
   la RAM (DDR4)       ~0,0001 ms         ~40-50 GB/s
```

El disco sigue siendo mil veces mas lento que la RAM en la primera lectura,
pero entre que el raton se para sobre algo y el clic pasan unos 200-300 ms
(lo que miden los que precargan enlaces). En ese hueco el SSD SATA del
Ryzen trae ~100 MB, y un NVMe varios GB: una funcion entera llega antes que
el dedo. [!] Hoy el disco de BMO-X usa 1 de sus 32 colas (`queue: 1 de 32`
en el informe): con varias a la vez (NCQ) lee mas en el mismo hueco.

---

## 3. Lo que puede salir mal, dicho antes

- **Un fallo de pagina dentro de una puerta del kernel** (el kernel lee
  memoria de la app que aun no existe): cada puerta que copia de Ring 3
  tiene que tolerarlo o tocar antes. Es el riesgo mayor; por eso EL1 va
  primero y solo con ceros.
- **La 3060 y el DMA no fallan con gracia**: una pagina prestada a un
  aparato (C1, la IOMMU) se FIJA y no entra en el barrido nunca.
- **Tiempo real**: el audio y el escritorio no pueden esperar a un fallo.
  Sus paginas se fijan igual (las que tocan el orquestador y el compositor).

---

## 4. De donde sale

- Paginacion bajo demanda y conjunto de trabajo: Denning, *The Working Set
  Model for Program Behavior* (1968); el algoritmo del reloj (bit A) es de
  Corbato (Multics, 1968). Linux (`mm/memory.c`, `handle_mm_fault`) y el
  recorte de Windows (working set trimming) son la misma idea. Citado de
  memoria: se lee antes de escribir la primera linea.
- Lo de BMO-X: `ring0/mm/vmm`, el censo de laminas (A1 de EL_FOCO), la
  lente al admitir (B1), y el informe del Ryzen del 15-10 (capitulo 5, la
  ficha BEF2: `fichero` frente a `mapeado`).
