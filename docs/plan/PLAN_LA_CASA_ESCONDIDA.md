# PLAN LA CASA ESCONDIDA -- una red que cambia, para que la IP de casa no se vea

> Escrito el **2026-10-03**. El propietario: *"que mi red sea cambiante total
> para proteger la IP personal de la casa, como estilo Tor pero extremo,
> tomando inspiracion"*.
>
> Este plan dice primero QUIEN ve hoy la IP de casa y por que, despues que se
> puede y que no se puede cambiar, y al final los escalones. Va encima de
> `PLAN_LA_RED_SOLA.md` (la red que se maneja sola y es CELOSA) y de
> `PLAN_HERMES.md` (H4, H6, H11).

---

# 0. LA RESPUESTA CORTA

```text
   la IP PUBLICA     la pone el proveedor en el router. BMO-X no la elige ni
                     la cambia: lo que SI decide es por donde SALE cada cosa,
                     y asi QUIEN la llega a ver
   quien la ve HOY   BMO-X: nadie de fuera (es celoso: router, DNS, antena,
                     amigos). La ANTENA: Google y YouTube, cada vez que el
                     movil navega por el WiFi de casa
   "cambiante"       tres capas que cambian solas: la MAC y el DHCP en la LAN
                     (cada arranque), el CAMINO de cada mensaje de HERMES
                     (cada mensaje), y la SALIDA de la antena (datos moviles,
                     un tunel, o Tor), elegida por el propietario
   "Tor extremo"     Tor esconde la IP pero se le puede ver el RITMO: quien
                     mira las dos puntas casa los tiempos. Una RED DE MEZCLA
                     (Loopix, la que usa Nym) no: paquetes de la misma medida,
                     cada uno por su camino, retrasos al azar y trafico de
                     cobertura que no se distingue del de verdad
```

*** **Lo que este plan NO promete.** El anonimato necesita MULTITUD: Tor
esconde porque miles de personas usan los mismos relevos. Una red privada de
tres amigos esconde la IP de cada uno a los OTROS y al que mira el cable,
pero no a alguien que mire toda la red a la vez. Se dice en cada escalon que
protege y de quien. Y no se inventa criptografia: lo que hace falta ya esta
en `bmo-cripto` (X25519, AES-GCM, HKDF, SHA-256) y en `bmo-hermes` (Noise).

---

# 1. QUIEN VE LA IP DE CASA, HOY (03-10)

| quien | la ve? | por que | lo que la tapa |
|---|---|---|---|
| el router y el proveedor | SI, siempre | son el camino | nada: es su trabajo |
| un servidor de fuera que BMO-X llame | NO hay ninguno | BMO-X es celoso (`PLAN_LA_RED_SOLA.md` 0) | se queda asi |
| Google, YouTube, las tiendas | SI, por la ANTENA en el WiFi de casa | el movil sale por el router de casa | CE5: otra salida |
| un amigo de HERMES (H6) | SI, si se hablan directo | TCP de punta a punta | CE2-CE4: relevos |
| alguien en la LAN de casa | ve la MAC de BMO-X, fija | la tarjeta trae una de fabrica | CE1: MAC que cambia |
| el repositorio publico | NO | guardian `privacidad` (ni MAC ni IP) | ya esta |

---

# 2. LO QUE SE TOMA DE CADA UNO (como TECNICA; nada se copia)

```text
   de TOR         el ruteo en CEBOLLA: cada salto quita una capa y solo sabe
   (y de Arti,    el anterior y el siguiente; nadie sabe las dos puntas.
   el Tor en      Arti 2.6 (sep 2026) ya es cliente y servicio onion, aun
   Rust)          no relevo
   de LOOPIX      lo que Tor no tiene: (1) PAQUETES SPHINX de medida fija,
   (Nym)          capa por capa; (2) cada paquete por su CAMINO, no un
                  circuito largo; (3) RETRASOS al azar (Poisson) en cada
                  salto; (4) TRAFICO DE COBERTURA: paquetes falsos,
                  indistinguibles, que salen siempre al mismo ritmo -- el
                  que mira ve el mismo goteo haya o no mensaje
   de la RED      un relevo PROPIO para HERMES (H11) que no tiene ninguna de
   de la casa     las dos llaves; y la ANTENA como salida para lo de fuera
```

El formato de paquete: **Sphinx**, y no uno inventado. Tiene una variante
publica con VECTORES DE PRUEBA oficiales (la cebolla de Lightning, BOLT 4),
contra los que el crate se compara byte a byte, como se hizo con los
vectores de Noise en H2.

---

# 3. LAS TRES COSAS QUE CAMBIAN SOLAS

```text
   capa           que cambia                       cada cuanto       protege de
   -------------  -------------------------------  ----------------  ----------------------
   la LAN         la MAC de BMO-X (administrada    cada arranque     quien mira la LAN, los
                  localmente, al azar) y el id                       registros del router
                  del cliente DHCP; ningun nombre
   HERMES         el CAMINO de cada paquete por    cada paquete      el amigo, el relevo y
                  los relevos, y las llaves de                       el que mira el cable
                  cada sesion (Noise)
   la SALIDA de   por donde sale el movil: WiFi    cuando el         Google y YouTube: ven
   la antena      de casa, DATOS MOVILES, un       propietario la    otra IP, no la de casa
                  tunel o Tor                      cambia, o sola
                                                   cada N minutos
```

---

# 4. LOS ESCALONES

- [ ] **CE0 -- quien ve la casa, dicho.** Una seccion del `save` y la orden
      `red quien`: cada destino con el que BMO-X hablo en esta sesion (el
      router, el DNS, la antena, cada amigo), con que capa de este plan
      salio y si ese destino ve la IP de casa. Sin preguntar nunca la IP
      publica (regla de `PLAN_RED_TX.md`). **Como se sabe:** tras `red ip`,
      `red dns` y `red hola`, `red quien` lista tres destinos y los tres
      "de la casa".
- [ ] **CE1 -- la MAC y el DHCP que cambian** (LAN). Al arrancar, una MAC al
      azar con el bit de "administrada localmente" (la RTL8168 la acepta en
      `IDR0-5` con `Cfg9346` abierto, como la pone el r8169 de Linux), un `client-id` de DHCP nuevo y ningun nombre de maquina.
      El GRIFO (`bmo-net/src/tx.rs`) y el radar comprueban "la NUESTRA", que
      pasa a ser la de esta sesion, no la de fabrica. `red mac` dice las
      dos. **Como se sabe:** dos arranques, dos MAC y dos IP distintas en la
      lista del router; `red prueba` sigue en PASA.
- [ ] **CE2 -- HERMES por un relevo PROPIO** (es H11). Los amigos no se
      conectan entre ellos: cada uno al relevo, que reenvia trozos cifrados
      de punta a punta (Noise) sin tener ninguna llave. El amigo no ve tu IP;
      el relevo ve las dos IP pero ni una palabra. **Como se sabe:** el de
      H11, y una captura en el amigo no tiene tu IP.
- [ ] **CE3 -- dos saltos: la CEBOLLA.** Dos relevos en fila, y cada uno solo
      conoce a su vecino: ninguno sabe a la vez quien manda y a quien.
      `bmo-velo` (nombre provisional), puro y con banco, encima de
      `bmo-cripto`: la cebolla de capas con X25519 + HKDF + AES-GCM, y el
      formato Sphinx comprobado contra los vectores de BOLT 4. **Como se
      sabe:** `cargo test -p bmo-velo` pasa los vectores byte a byte; un
      bit tocado en cualquier capa tumba el paquete; 100.000 paquetes
      hostiles no lo hacen caer.
- [ ] **CE4 -- la MEZCLA: medida fija, retrasos y cobertura** (lo "extremo",
      Loopix). Todos los paquetes de la misma medida; cada relevo los retiene
      un tiempo al azar (Poisson) antes de soltarlos; y BMO-X manda paquetes
      de COBERTURA a un ritmo fijo, que vuelven a el en lazo, indistinguibles
      de los de verdad. Con PRESUPUESTO: el goteo se dice en `[consumo]` (es
      red, CPU y vatios de verdad) y el propietario lo enciende. **Como se
      sabe:** con y sin mensajes, una captura del cable de casa da el mismo
      numero de paquetes por segundo y de la misma medida.
- [ ] **CE5 -- la SALIDA de la antena, elegida y DICHA.** Lo de fuera (Google,
      YouTube, las tiendas) lo pide la ANTENA, y la antena puede salir por:
      el WiFi de casa (Google ve la casa), los DATOS MOVILES (ve la IP del
      operador, compartida por miles con CGNAT, y cambia sola), un tunel
      propio, o Tor. En BMO-X, un piloto dice POR DONDE salio la ultima cosa
      -- no la IP, la CLASE. **Como se sabe:** con la salida en "datos", la
      antena pide una pagina y el piloto dice "fuera de casa"; el router no
      registra trafico del movil hacia fuera.
      [!] **YouTube y Tor no se llevan:** ~95 % de las salidas de Tor estan
      en su lista y piden "confirma que no eres un bot". Para YouTube la
      salida util es DATOS MOVILES o un tunel; Tor queda para lo demas.
- [ ] **CE6 -- la rotacion sola.** El VIGIA (`PLAN_LA_RED_SOLA.md` RS1) cambia
      los caminos de HERMES y, si el propietario lo pidio, la salida de la
      antena cada N minutos, y lo apunta (CE0). **Como se sabe:** `red quien`
      muestra la rotacion en la hora.

---

# 5. LO QUE DECIDE EL PROPIETARIO

- **D1 -- Arti en la ANTENA?** Tor de verdad, en Rust y del proyecto Tor, en
  el MOVIL (nunca en BMO-X). Choca con la regla de la LUDOTECA ("ni un crate
  de fuera, ni en la antena"): o se hace una excepcion con motivo, o la
  salida por Tor queda fuera y CE5 se queda con datos moviles y tunel.
- **D2 -- donde vive el relevo** (CE2): un servidor alquilado (alguien lo
  aloja, y ve las IP de los dos), el BMO-X de un amigo de confianza, o la
  antena con datos moviles. Ninguno es gratis en privacidad; se elige
  sabiendolo.
- **D3 -- el precio de la cobertura** (CE4): cuanto goteo al dia. Mas
  goteo, mas protege, y mas datos y vatios gasta.
