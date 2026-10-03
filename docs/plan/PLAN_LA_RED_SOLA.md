# PLAN LA RED SOLA -- la red que sabe, que se maneja y que avisa, sin Google

> Escrito el **2026-10-03**. El propietario, despues de HERMES en F3:
>
> > *"prepara el internet, ese mismo es el motivo, pero OJO no quiero como
> > Google o Chromium [...] BMO-X y Google no se quieren mucho :3 [...]
> > que seria completar red pero automatico? que sepa, que maneje y todo
> > eso de forma maestra. Analizas todo el plan en docs y METAS?"*
>
> Este plan es esa respuesta. Se escribio despues de leer, entero, lo que hay
> de red: `METAS.md` seccion 5, `PLAN_RED_TX.md`, `PLAN_NAVEGAR.md`,
> `PLAN_CLOUD_LOCAL.md`, `PLAN_HERMES.md`, `PLAN_SEGURIDAD.md`,
> `PLAN_LA_LUDOTECA.md` (la escalera R1-R6), `EL_ORDEN.md`,
> `docs/maestro/RED_MAESTRO.md` y las hojas del metal; y el codigo: `bmo-net`,
> `ring0/red`, `bmo-puerta-red`, `bmo-pila`, `bmo-cripto`, `bmo-antena`,
> `bmo-hermes` y las ordenes `red *` del DIRECTOR.

---

# 0. LA RESPUESTA CORTA

```text
   HOY        la red FUNCIONA a mano: cada cosa es una orden (`red ip`,
              `red ping`, `red dns`, `red hola`), cada orden abre un pase de
              60 s y 50 tramas, lo usa y lo cierra. Entre orden y orden la
              maquina no existe en la red: no contesta ni a un ARP
   SOLA       la red se ENCIENDE una vez (`red = si` en sys/director.cfg) y
              desde ahi se lleva ella sola: ve el cable, pide su IP, la
              RENUEVA antes de que caduque, vigila el router, se reconecta
              si el cable vuelve, guarda lo que aprendio para el siguiente
              arranque, y lo DICE (un piloto en el panel, un aviso que suena,
              y `red` cuenta en que paso esta y por que)
   SIN GOOGLE ninguna pieza de esta red habla con Google ni lo necesita: no
              hay "comprobar conexion" contra un servidor de nadie, no hay
              navegador, no hay DNS de terceros puesto a fuego. Estar "en
              linea" se mide contra TU router y TU servidor de nombres
   EL MOTIVO  HERMES: dos BMO-X que se hablan. La red sola es el suelo de
              H4 (la PUERTA), H6 (la LAN) y H11 (fuera de casa)
```

**Y CELOSA (03-10, el propietario):** *"mi BMO-X es ultra celoso en RED, eso
tiene que aplicarse; ya si es algo mas complejo, mi ANTENA es el que lleva
todo el peso y la responsabilidad, por algo tengo navegador para eso: para ir
a ver Google en la ANTENA sin mi BMO-X"*. Y: *"BMO-X coma RED en el router
principal con fuerza, lo que este conectado al cable; el wifi no lo ponemos"*.
O sea, la lista ENTERA de con quien habla BMO-X:

```text
   el ROUTER          ARP y DHCP, por el cable (sin wifi: no hay, y no se pone)
   el DNS             el que da el router (D4), solo para nombres de la casa
   la ANTENA          el movil o el PC, en la LAN, emparejada (P0). Ella lleva
                      TODO lo de fuera: Google, las tiendas, su navegador, los
                      logins, los tokens, el TLS. BMO-X recibe DATOS juzgados
   los AMIGOS         HERMES: dos BMO-X por huella (H6), y su relevo (H11)
   nadie mas          ni un "comprobar conexion", ni una hora de internet, ni
                      telemetria, ni una actualizacion sola. Lo que no esta en
                      esta lista lo tira el VIGIA y lo dice con nombre
```

*** **Automatica no es abierta.** Lo que se automatiza es MANTENER la red
que el propietario encendio, no decidir por el. La regla de la LUDOTECA sigue:
*"internet es un interruptor, cerrado de serie"* (`PLAN_LA_LUDOTECA.md`
249-256). Y la del codigo: *"el kernel no sabe lo que es una IP"*
(`docs/maestro/RED_MAESTRO.md` 83-98) -- todo lo de este plan vive en Ring 3, salvo
dos piezas de Ring 0 dichas con su motivo (RS2 y RS5).

---

# 1. LO QUE HAY, MEDIDO (03-10)

| capa | estado | donde | metal |
|---|---|---|---|
| tarjeta RTL8168, RX | hecho | `platform/drivers/net`, `ring0/red/mod.rs` | 13-09: 16 tramas, 0 perdidas |
| TX con grifo y vuelos | hecho | `bmo-net/src/tx.rs`, `ring0/red/salida.rs` | 14-09: `red prueba` PASA |
| la PUERTA RED (pase, buzon, radar) | hecho | `bmo-puerta-red`, `ring0/red/puerta.rs` | 14-09 |
| ARP, IPv4, ICMP, UDP | hecho | `bmo-pila` (76 pruebas) | 14-09: ping al router e internet |
| DHCP | hecho, SIN renovar | `bmo-pila/src/dhcp.rs` | 14-09: CONCEDIDA, 2 h |
| DNS (A) | en codigo | `bmo-pila/src/dns.rs` | falta la foto (G4) |
| TCP | en codigo | `bmo-pila/src/tcp` | espera el Ryzen (G5) |
| cripto | SHA-256/512, HMAC, HKDF, X25519, AES-GCM, Ed25519 (verificar) | `bmo-cripto` (56 pruebas) | -- |
| HERMES/1 (Noise) | hecho, sin cable | `bmo-hermes` (29 pruebas) | -- |
| TLS 1.3, X.509, ChaCha20 | NO EXISTEN | -- | -- |
| IPv6 | rechazado a proposito | lista blanca de `bmo-pila` | -- |

**Lo que NO hay y una red sola necesita** (de la lectura, con su sitio):

1. **Un titular que dure.** Hoy hay UN pase en todo el sistema, de 10 min y
   10.000 tramas como mucho (`bmo-net/src/tx.rs`, `DURACION_MAX`/`CUPO_MAX`),
   y cada orden abre el suyo de 60 s y 50 tramas (`red_nodo.rs`). Una
   pagina larga por `red pagina` puede quedarse sin cupo a medias
   (`CupoGastado`). Una red sola pide un pase que se SOSTIENE, y un solo
   titular que reparta.
2. **DHCP entero.** Sin T1/T2 no se renueva ni se reengancha: a las 2 h la
   IP caduca en silencio (`dhcp.rs`, `concesion()` da `None`).
3. **El cable como suceso.** El enlace se lee en vivo (`PHYstatus`) y el radar
   revoca el pase si cae, pero nadie vuelve a empezar cuando el cable vuelve.
4. **Memoria.** La IP vive en la memoria del DIRECTOR; no hay `sys/red.cfg`
   ni la ultima concesion guardada.
5. **Avisos.** Nada dice "conectado" o "sin red" sin teclear `red`.
6. **Interrupciones.** Todo es sondeo cada 4 ms (el MSI se DETECTA y no se
   enciende, `ring0/red/mod.rs` 680-707): los tiempos salen en escalones de
   16 ms y el TCP no puede medir su RTT.
7. **El muro.** Sin IOMMU para la tarjeta es contabilidad, no muro (E4). La
   AMD-Vi ya anda en el Ryzen desde el 24-09, para la 3060.
8. **El enlace a 10 Mbit** desde el 24-08 (`PHYstatus` 0x87), que el 12-08
   era de 100. Sin diagnostico.
9. **Un TCP de diario**: sin RTT medido (RFC 6298), sin cola de desordenados,
   y la ISN de `red_tcp.rs` sale de ciclos ^ MAC en vez de `bmo_cripto::azar`.

---

# 2. LAS REGLAS (las que ya estaban, y que este plan no toca)

1. **Los protocolos son de Ring 3.** El kernel entrega el cable y se aparta.
2. **Lista blanca y rechazo con nombre** (`bmo-pila/src/lib.rs` 15-25): lo que
   no se entiende no se adivina, se tira y se dice por que.
3. **RED es una autoridad de nacimiento**, no una capacidad que se pasa: la
   pone Ring 0, no se hereda, y una app lanzada desde Ring 3 nace con NINGUNA
   (`task/autoridad.rs` 56-84). No es una jerarquia y no se convierte en una.
4. **Privacidad**: ni la MAC entera ni una IP de la casa en el repo; la IP
   publica no se pregunta nunca (guardian `privacidad`).
5. **Datos, nunca codigo**: lo que llega se juzga como DATO; un `.bex` solo si
   viene FIRMADO.
6. **La via directa habla protocolos que caben en una pagina de RFC; NUNCA
   JavaScript, NUNCA un navegador** (`PLAN_CLOUD_LOCAL.md` 768-776).
7. **HERMES y la ANTENA no comparten nada**: ni crate, ni puerto, ni relevo.
8. **Sin crates de fuera** (`PLAN_LA_LUDOTECA.md` 167-170). smoltcp, que un
   documento viejo recomendaba, queda SUPERADO por `bmo-pila`.
9. **Cada escalon con su prueba**, y la primera trama que sale no es la
   primera vez que corre el codigo que la arma (`PLAN_RED_TX.md` 10-12).

---

# 3. COMO SE ARMA: EL VIGIA DE LA RED

```text
   Ring 0     el cable (bmo-net), el grifo, la PUERTA y su radar   [ya esta]
              + el pase que se SOSTIENE (RS2) + el MSI (RS5)
                         | buzon de 7 paginas, un solo pase
   Ring 3     EL VIGIA: `bmo-red-sola`, puro y con banco, dentro de quien
              tenga el pase (hoy el DIRECTOR; despues su propio servicio)
                - el ENLACE    sube / baja / vuelve
                - la DIRECCION DHCP entero, o fija del fichero
                - el ROUTER    vivo? (un ARP cada tanto, sin internet)
                - los NOMBRES  cache de DNS con su TTL
                - la SALUD     un estado y el porque, en una linea
                - el REPARTO   quien usa la red: ordenes, HERMES, la antena
                         | bmo-cola
   clientes   las ordenes `red *`, la PUERTA HERMES (H4), el antenista
```

**El VIGIA es una maquina de estados que no hace E/S**: recibe el tiempo,
las tramas y lo que lee del enlace, y devuelve tramas que mandar y sucesos
(`Conectado { ip }`, `SinCable`, `Renovada`, `RouterMudo`...). Asi se prueba
entero en el anfitrion, con una red de mentira que corta el cable, pierde
paquetes y miente, antes de que el Ryzen la vea -- la misma regla con la que
se hizo `bmo-pila`.

```text
   APAGADA --(red = si)--> SIN CABLE --(enlace)--> PIDIENDO (DHCP / fija)
      ^                       ^   |                    |
      |                       |   '----(cae)-----------+
   (red = no)                 |                        v
      |                  (cae el cable)            CONECTADA --T1--> RENOVANDO
      '---------------------- + ---------------------- | <--ACK----'    |
                                                       |              T2
                                    (router mudo 3x)   v               v
                                                  SIN SALIDA     REENGANCHANDO
                                                (vuelve a pedir)  --NAK/fin--> PIDIENDO
```

Cada flecha es un SUCESO que se cuenta: al panel (el piloto), al oido (los
avisos de la VOZ de BMO-X que ya existen: `conecta` al conectar, `seva` al
perder el cable, `error` si el router no contesta) y a CABINA.

---

# 4. LOS ESCALONES

Ordenados por lo que DESBLOQUEA y por lo que MIENTE hoy (`EL_ORDEN.md`).

- [~] **RS0b y RS5b, EN CODIGO el 03-10: los 10 Mbit, explicados y con su
      arreglo.** El propietario: *"mi Internet es de 100, el router no es el
      cuello de botella: algo lo limita"*. Lo que se encontro: BMO-X NUNCA
      escribia en el PHY, y el driver de Realtek de Windows, al APAGAR con
      Wake-on-LAN ("WOL & Shutdown Link Speed = 10 Mbps First", lo de
      fabrica), deja el PHY anunciando SOLO 10 Mbit. Windows renegocia al
      volver; BMO-X no lo hacia. Ahora:
      - `bmo_net::mii` (puro, 6 pruebas): los registros MII, los DOS caminos
        al PHY (`PHYAR` en los 8168 viejos, `GPHY_OCP` en los 8168g/h), el
        VEREDICTO (que anunciamos, que anuncia el router, que sale y por que:
        anunciamos poco, el router da poco, cable de dos pares, sin
        autonegociar, apagado, sin enlace) y lo que se escribe para anunciar
        10/100/1000 y renegociar;
      - el kernel prueba los dos caminos y se queda con el que contesta el
        fabricante de Realtek (`PHYID1 = 0x001C`), no lo supone;
      - `RED_OP_MII` (leer, libre; `0xFF` = el veredicto) y
        `RED_OP_RENEGOCIAR` (pide la autoridad RED);
      - `red phy` dice el veredicto y `red velocidad` renegocia.
      **Como se sabe:** en el Ryzen, `red phy` dice "anunciamos 10, el router
      100: ANUNCIAMOS POCO"; tras `red velocidad`, a los 5 s, `red` dice 100
      (o 1000) Mbit. Prueba de la sospecha sin tocar BMO-X: reiniciar desde
      Windows (no apagar) y ver si `red` ya sale a 100.
- [ ] **RS0 -- la tanda del Ryzen que ya espera.** G4 (`red dns`), G5 (`red
      hola` contra `antena.py`) y la causa de los 10 Mbit, en la misma
      sesion. Para los 10 Mbit: leer los registros del PHY (BMCR, ANAR,
      ANLPAR por `PHYAR`) y DECIR que se negocio y por que; sin escribir nada
      todavia. **Como se sabe:** las tres fotos en una hoja de `docs/metal/`:
      la IP de un nombre igual a la de Windows, `CIERRE LIMPIO`, y una linea
      "anuncio 100/1000, el otro lado ofrece X" que explica el 10.
- [ ] **RS1 -- EL VIGIA, puro y con banco.** `platform/shared/bmo-red-sola`,
      `no_std`, sin `unsafe`, encima de `bmo-pila`: la maquina de estados de la
      seccion 3 con DHCP ENTERO (T1 = 50 %, T2 = 87,5 %, INIT-REBOOT con la
      ultima concesion, RFC 2131), anuncio y prueba de la IP por ARP antes de
      usarla (RFC 5227, en chico: si otro la tiene, se dice y se pide otra),
      el router vigilado por ARP cada 30 s, la cache de DNS con TTL, y la
      espera creciente (1-2-4-8-60 s) cuando algo falla. **Como se sabe:**
      `cargo test -p bmo-red-sola` con una LAN de mentira: el cable se corta y
      vuelve y la IP se recupera; una renovacion sin respuesta pasa a
      reenganche y despues a pedir; un ARP que reclama nuestra IP la suelta;
      el reloj salta 3 h y nada se queda colgado; y 50.000 tramas hostiles no
      la tumban.
- [ ] **RS2 -- el pase que se SOSTIENE** (Ring 0, chico y con motivo). Un
      pase con `RED_OP_ABRIR` que el titular RENUEVA antes de caducar
      (`RED_OP_RENOVAR`): el radar sigue mirando el RITMO, la MAC y los indices
      igual que hoy, y el cupo pasa de "total" a "por minuto". Sin esto, una
      red sola cae a los 10 minutos. **Como se sabe:** un pase abierto una hora
      sin revocar, y el radar sigue revocando una inundacion en 100 ms
      (`bmo-puerta-red`, pruebas nuevas, y el Ryzen).
- [ ] **RS3 -- el VIGIA en el DIRECTOR, encendido por fichero.** `red = si`
      (y `red_ip = auto` o `a.b.c.d/24 router dns`) en `sys/director.cfg`, de
      serie `no`. Con `si`, el DIRECTOR abre el pase al arrancar, el VIGIA lo
      lleva, y las ordenes `red ip/ping/dns/hola/pagina` dejan de abrir su
      pase: PIDEN al VIGIA. La ultima concesion se guarda en `sys/red.txt`
      (en la maquina, nunca en el repo) para el INIT-REBOOT. **Como se sabe:**
      en el Ryzen, arrancar con `red = si` y sin teclear nada: a los pocos
      segundos `red` dice `CONECTADA, IP de la casa, renueva en 59 min`; el
      cable fuera y dentro, y vuelve sola.
- [ ] **RS4 -- QUE SEPA Y QUE LO DIGA.** El piloto de la red en el panel
      (apagada / sin cable / pidiendo / conectada / sin salida), los avisos
      con su sonido (`conecta`, `seva`, `error`) y su sitio en 3D, y `red`
      reescrito como un DIAGNOSTICO de arriba abajo: que paso falla, desde
      cuando, y que hacer ("el router no contesta a ARP desde hace 2 min: mira
      el cable o reinicia el router"). En el `save`, una seccion de red con
      los sucesos del arranque. **Como se sabe:** el propietario quita el
      cable con la musica sonando: suena `seva`, el piloto se apaga, y al
      ponerlo suena `conecta` sin tocar el teclado.
- [ ] **RS5 -- la tarjeta avisa (MSI)** (Ring 0). Encender el MSI que ya se
      detecta, con el `IMR` de RX y de enlace; el latido de 4 ms queda de red
      de seguridad. **Como se sabe:** `red ping` al router da tiempos por
      debajo del milisegundo en vez de escalones de 16, y el cambio de cable
      se ve al instante (el radar lo dice en CABINA).
- [ ] **RS5b -- EL CABLE A FONDO: el gigabit.** La RTL8168 es de 1000
      Mbit y el enlace lleva a 10 desde el 24-08. Con lo de RS0 dicho, se
      reinicia la autonegociacion anunciando 1000/100/10 (BMCR, ANAR y
      GBCR por `PHYAR`), y los anillos crecen de 16 a 256 descriptores con
      su corral (`bmo-net/src/anillo.rs` ya lo calcula para cualquier
      medida). "Con fuerza" se mide: **Como se sabe:** `red` dice `1000
      Mbit, duplex completo`, y una transferencia larga por la LAN pasa de
      100 MB/s sin una trama perdida (`MPC` a cero).
- [ ] **RS5c -- LA RED EN SU PROPIO NUCLEO, en tiempo real.** El
      propietario: *"que el CPU tenga su core y su hilo en tiempo real que
      jale fuerte, para que use todo; no tener cuello de botella al
      cargar"*. Como el USB (`PLAN_EL_BUS_APARTE.md` A2): la tarjeta, el
      latido de la PUERTA y el VIGIA viven en un nucleo RESIDENTE elegido
      por perfil, con su CONTRATO del COMPAS (periodo y presupuesto,
      `PLAN_EL_COMPAS.md`), y su `[consumo]` dicho en `save`. Lo que llega
      se reparte: el nucleo de la red recibe y ordena, OTROS nucleos
      comprueban las sumas (SHA-256 con SHA-NI, que el Ryzen tiene y el
      kernel ya detecta, `cpu_vendor/features`) y ESTRATOS escribe. A 1000
      Mbit son 125 MB/s; la suma por SHA-NI va muy por encima, asi que el
      cuello no esta en la CPU si se reparte bien. **Como se sabe:** una
      descarga por la antena a 1000 Mbit con DOOM corriendo a sus fps de
      siempre, y `save` dice cuanto gasto el nucleo de la red y cuanto
      espero cada trozo.
- [ ] **RS6 -- el muro: la tarjeta detras de la IOMMU** (es E4). La AMD-Vi
      ya traduce para la 3060 en el Ryzen; se pone a la RTL8168 en su propio
      dominio con SOLO su corral de RX y su anillo de TX. **Como se sabe:** una
      escritura de la tarjeta fuera de su corral da un fallo de pagina de la
      IOMMU con nombre, y la red sigue funcionando. *** Antes de RS9 no se
      habla con nadie de fuera de casa.
- [ ] **RS7 -- un TCP de diario.** RTT medido (RFC 6298), la cola de
      desordenados, la ISN con `bmo_cripto::azar` (hoy ciclos ^ MAC en
      `red_tcp.rs`: se arregla YA, es una linea), y VARIAS conexiones a la vez
      por el VIGIA. **Como se sabe:** una transferencia de 10 MB por la LAN sin
      un byte cambiado y con su velocidad medida; dos conexiones a la vez.
- [ ] **RS8 -- su propio proceso.** El VIGIA sale del DIRECTOR a
      `services/red`, nacido de Ring 0 con autoridad RED (y nada mas), como la
      PUERTA HERMES (H4) pide. Los clientes le hablan por `bmo-cola`. Pide
      una DECISION (seccion 5, D2). **Como se sabe:** el DIRECTOR se cae y la
      red sigue conectada; HERMES, con autoridad NINGUNA, manda un mensaje.
- [ ] **RS9 -- HERMES en la red** (son H4 y H6). Dos BMO-X en la misma LAN se
      aceptan por huella y se escriben con Noise, por el VIGIA. **Como se
      sabe:** el de H6 -- una captura del cable no tiene ni una palabra en
      claro.
- [ ] **RS10 -- fuera de casa, sin Google** (es H11; lo demas de fuera,
      las tiendas y la web, lo lleva la ANTENA: ver `PLAN_LA_LUDOTECA.md`
      seccion 3c). Un relevo HERMES PROPIO
      (no la ANTENA, no un servidor de nadie) que no tiene ninguna de las dos
      llaves; sin UPnP (abrir puertos del router desde dentro es justo lo que
      no se hace). **Como se sabe:** el de H11.

Y el **TLS 1.3**: con la decision D1, el TLS de las tiendas vive en la
ANTENA (Rust, la escalera R3-R6 de la LUDOTECA), no en BMO-X. BMO-X no
necesita TLS para nada de esta lista: con la antena habla por la LAN
emparejado (P0), y con sus amigos por Noise (HERMES). El **IPv6** sigue
rechazado: la red de la casa no lo exige.

---

# 5. LO QUE DECIDE EL PROPIETARIO

- [x] **D1 -- la ANTENA usa Chromium: SI, y es su trabajo.** DECIDIDO el
  03-10 por el propietario: *"si es algo mas complejo, mi ANTENA es el que
  lleva todo el peso [...] por algo tengo navegador para eso, para ir a ver
  Google en la ANTENA sin mi BMO-X"*. Chromium, Google y los logins de las
  tiendas viven en la antena (`toolchain/tools/antena/navegador.py`); a
  BMO-X solo le llegan sus RESULTADOS juzgados (laminas, lineas de la
  LUDOTECA, ficheros con su suma). Ver la lista de la seccion 0.
- **D2 -- quien da la autoridad RED a un servicio** (RS8). Hoy solo Ring 0 la
  pone al nacer, y el DIRECTOR (con RED) no puede darsela a un hijo. Dos
  caminos: que el KERNEL lance `services/red` en el arranque (como lanza el
  DIRECTOR), o una orden `TASK_OP_AUTORIDAD_HIJO` acotada a RED, que solo
  puede usar quien la tiene. La primera no cambia la regla 3; la segunda la
  estira.
- **D3 -- encendida de serie o no.** Este plan propone `red = no` de serie
  (la regla de la LUDOTECA). Con `si` de serie, el Ryzen estaria en la red
  desde el primer arranque.
- **D4 -- el servidor de nombres.** El que da el router por DHCP (propuesta),
  o uno fijo en el fichero. Ninguno puesto a fuego en el codigo, y ninguno de
  Google.

---

# 6. LO QUE LA LECTURA ENCONTRO TORCIDO (para arreglar en los planes)

- `PLAN_RED_TX.md`: E5 sigue `[ ]` y G3 `[x]` dice "es E5".
- `METAS.md` seccion 5 decia "falta la foto del ARP contestado", y el ARP se
  contesto en el Ryzen el 14-09 (`red prueba` PASA). Corregido con este plan.
- DNS: "hecho en metal" en `PLAN_CLOUD_LOCAL.md` 799 y en el README, y G4
  `[ ]` en `PLAN_RED_TX.md`. Manda el plan: falta la foto (RS0).
- TLS: "juego completo en primitivas" (`EL_ORDEN.md` 278-280) contra G6, que
  pide ChaCha20 (no esta) y la LUDOTECA, que pide RSA, P-256 y X.509 (no
  estan). Lo cierto: hay AES-GCM, X25519 y HKDF; no hay TLS.
- `docs/maestro/RED_MAESTRO.md` describe los anillos de la tarjeta mapeados en
  Ring 3 (sin copia); lo que se construyo copia por el buzon. Y varios
  comentarios viejos dicen "esto no transmite" (`commands/red.rs` 27-29,
  `ring0/red/mod.rs` 37, `bmo-net/Cargo.toml`).
- `PLAN_HERMES.md` H4 pide un servicio con RED que compite con el UNICO pase
  del DIRECTOR: lo resuelve RS8 (un solo titular que reparte).
- smoltcp recomendado en `terminado/PLAN_EL_PERFIL_TOTAL.md` 120-139: SUPERADO
  por `bmo-pila` y la regla de independencia.
- La maquina es un **Ryzen 5 5600X** en todos los documentos; el "Ryzen 7
  5700X" de la maqueta de HERMES es el de "nova", un amigo de mentira.
