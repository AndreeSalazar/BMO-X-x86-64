# PLAN -- LA LENTE: BMO-X lee cada programa y dice que es y por que

> La idea, con las palabras del propietario, **2026-10-12** (el dia que
> Cyberpunk 2077 arranco por primera vez en BMO-X, por la CPU):
>
> *"que mi kernel lea todo ... como .txt pero con lectura que lees con kernel
> que te dice porque y razones y motivos porque no acepta, que hay trampa,
> exponiendo todo hasta posible potencial que quiere perjudicar mi BMO-X"* y
> *"mi kernel se convierte mi lente para leer todo que son y porque"*.

---

# 1. QUE ES

Antes de que algo corra, BMO-X lo **LEE** (sin ejecutarlo) y escribe un
informe en texto: que es, que pide, que esconde y, si dice que NO, la regla y
el motivo. Mientras corre, cada NO del kernel se apunta con su porque. Es lo
que ya hacen tres piezas, cada una en lo suyo, juntado en una lente:

```text
   el juez de la 3060     lee cada sombreador sin correrlo, regla por regla
                          (R0..R7), y un NO dice cual y por que
   admitir.rs             que bytes se vuelven codigo y con que permisos
                          ("el permiso lo da el hueco")
   rayosx / PROTON-X      lo que un .exe o un ELF pide de fuera, y el
                          veredicto (PE32+, AMD64, codigo maquina); el diario
                          y el censo, lo que paso mientras corria
```

Lo que el informe diria (un ejemplo, no una promesa de formato):

```text
   programa.exe -- LEIDO, no ejecutado
   pide       red (12 funciones), archivos (30), otros procesos (2)
   esconde    una seccion cifrada de 4 MiB (lo que hara no se puede leer)
              codigo que se escribe a si mismo (W+X en VirtualProtect)
   intenta    leer la memoria de otro proceso; instalar un servicio
   veredicto  NO -- regla L3: "nadie toca la memoria de otro"
              (la importacion 17: ReadProcessMemory)
```

# 2. LO QUE NO ES (dicho antes, para no prometer de mas)

- **Leer no es saber la intencion.** La lente dice lo que un programa PUEDE
  hacer y lo que ESCONDE; lo que QUIERE solo se ve al correr. Lo empaquetado
  o cifrado se dice como tal ("no se puede leer"), no se adivina (NUNCA
  ADIVINA). Por eso hay dos mitades: antes (lo que pide) y durante (cada NO,
  con su motivo).
- **El lector es lo primero que un atacante apunta.** Un fichero hecho a
  proposito para romper al que lo lee. La lente vive en **Ring 3**, pura y
  sin `unsafe` (como el juez y `rayosx`); el kernel solo HACE CUMPLIR el
  veredicto. Si un fichero malo rompe al lector, se cae el lector, no BMO-X.
- **"Trampa" es una LISTA CERRADA de reglas del propietario**, no una
  sospecha. Cada una con nombre y motivo, como el juez. Lo que no esta en la
  lista no se bloquea a ciegas.

# 3. LAS CASILLAS

- [ ] **LE0 -- las reglas, escritas.** La lista cerrada, cada una con su
      nombre, su motivo y que la dispara. Las primeras candidatas: tocar la
      memoria de otro proceso; memoria escribible y ejecutable a la vez;
      codigo cifrado o empaquetado; cargar drivers o servicios; hablar con la
      red sin permiso; esconderse del depurador; pedir el teclado de otro.
      **Como se sabe:** la lista, aprobada por el propietario.
- [ ] **LE1 -- la lectura de un PE/ELF, en texto.** Un lector puro en Ring 3
      (lo de `rayosx` y el veredicto de PROTON-X) que saca el informe a
      `informe/lente/<programa>.txt` SIN ejecutar nada: secciones y sus
      permisos, importaciones por familia, la entropia de cada seccion (lo
      cifrado), y cada regla de LE0 con SI/NO y su motivo. **Como se sabe:**
      su banco con binarios hechos a proposito, uno por regla, y la prueba
      del NO (un fichero roto no tira al lector: dice "no se sostiene").
- [ ] **LE2 -- el veredicto, en la puerta.** Lanzar un programa pasa por la
      lente: con un NO de una regla, no corre, y la pantalla dice la regla y
      el motivo; el propietario puede decir "si, lo se" (y queda escrito).
      **Como se sabe:** un programa de prueba de cada regla, parado con su
      motivo; uno limpio, que corre igual que antes.
- [ ] **LE3 -- el diario de los NO, mientras corre.** Cada vez que el kernel
      niega algo a un programa (una syscall, memoria, un aparato), una linea
      con el quien, el que y el porque, en el mismo informe de la lente.
      **Como se sabe:** el informe de una sesion con Cyberpunk lleva los NO
      del kernel con sus motivos (lo que hoy es `cabina fallos` y el motivo
      79 de la 3060, en una lectura).
      **Primer trozo (12-10), solo la 3060:** cada NO de su puerta apunta su
      porque (`bmo_gpu_ga10x::porque`, `INFO_GPU_PORQUE`) y PROTON-X lo dice
      con palabras -- el ultimo, a quien lo recibio. Falta el diario entero
      (todos los NO, de todo el kernel, con quien) y que vaya al informe.
