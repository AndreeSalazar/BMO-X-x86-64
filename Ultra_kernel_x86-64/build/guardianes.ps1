# guardianes.ps1 -- los guardianes de Python que `build.ps1` corre antes de
# compilar (2026-10-03: salen de `build.ps1`, el texto tal cual).
#
# ** Por que es un fichero: con `bmo.ps1 -Paralelo` los corre la ventana
# VERIFICAR mientras la ventana COMPILAR construye (`build.ps1 -SinGuardianes`):
# lo que se comprueba y lo que se compila no se esperan el uno al otro. Y
# `build.ps1` a secas los sigue corriendo todos, en el mismo orden, antes de
# compilar: un build suelto no pierde ni uno.
#
# [!] Se carga con punto y usa `Guardian`, `Step` y `Fail` de `comun.ps1`.

# ---------------------------------------------------------------------------
# El idioma de las fuentes es un CONTRATO, igual que el de los syscalls, y por
# eso se comprueba en el mismo sitio y de la misma forma.
#
# No es estetica. Son dos fallos que ya se pagaron:
#
#   - El preprocesador de BMO C copiaba byte a byte. Una sola letra acentuada
#     en un literal hacia crecer el .bex de 512 a 492.032 bytes, y con MAX_BEX
#     en 1 MiB, dos palabras con tilde dejan un programa que ya no carga.
#   - La consola del kernel es Latin-1 A PROPOSITO --un byte por caracter, sin
#     decodificador-- y todo el camino de pintado entrega UTF-8 crudo con
#     `s.as_bytes()`. Una raya larga en una cadena del kernel pone TRES bytes
#     en pantalla donde iba un glifo.
#
# Los dos se arreglaron a mano el 2026-08-08. Esto es lo que impide el
# siguiente: sin esta comprobacion, la regla es una limpieza que hicimos una
# vez; con ella, es una propiedad del sistema. Arquitectura, no parche.
#
# Si no hay Python se AVISA y se sigue: un portico que no se puede levantar no
# debe cerrar la puerta. Pero si corre y falla, el build para.
Guardian 'Validating source encoding (sources are ASCII)' `
    'toolchain\tools\ascii-sweep\ascii_sweep.py' 'la codificacion' `
    'codificacion: hay no-ASCII donde la regla no lo permite (ver arriba)'

# ** La regla del 13-09 --ni IP de casa ni MAC entera en el repo publico-- se
# rompio el 18-09 sin que nada avisara: vivia solo en un texto. Ver su cabecera.
Guardian 'Validating no home IP or device MAC is published' `
    'toolchain\tools\privacidad\privacidad.py' 'la privacidad de la red' `
    'privacidad: hay una IP de casa o una MAC de fabricante en el repo (ver arriba)'

# ** CODEOWNERS es la lista de Ring 0 (cerrado el 17-09) y se desfaso en UN dia:
# `bmo-cola` entro en el kernel y la lista no se entero. Ver su cabecera.
Guardian 'Validating CODEOWNERS covers what the kernel links' `
    'toolchain\tools\codeowners\codeowners.py' 'el cerrojo de Ring 0' `
    'codeowners: la lista de Ring 0 no dice lo que el kernel enlaza (ver arriba)'

# ** Una arquitectura, un repositorio (2026-09-18): este es SOLO x86-64, y otra
# CPU es otro repositorio. Ver su cabecera.
Guardian 'Validating this repository is x86-64 only' `
    'toolchain\tools\isa\isa.py' 'que el repo sea de una sola arquitectura' `
    'isa: hay codigo, un target o una carpeta para otra CPU (ver arriba)'

# ** El metro del emisor de x86-64 (2026-09-18): instrucciones, bytes y la
# salida de un banco fijo de programas. Nada sube y ninguna salida cambia.
Guardian 'Validating the x86-64 emitters do not get worse' `
    'toolchain\tools\metro\metro.py' 'el metro del emisor' `
    'metro: un emisor emite mas, o una salida cambio (ver arriba)'

# ** LAS LEYES DE TITAN++ (2026-10-04), pedidas por el propietario: "que no se
# altere, con reglas y porque". Cada ley con su motivo y QUIEN la hace cumplir
# (un test vivo o un programa del banco); el texto no cambia sin `--sellar`
# con su motivo. Ver toolchain/tools/titan-leyes/LEYES.txt.
Guardian 'Validating the laws of TITAN++ are kept' `
    'toolchain\tools\titan-leyes\titan_leyes.py' 'las leyes de TITAN++' `
    'titan-leyes: una ley de TITAN++ perdio quien la cumple, o cambio sin sellar (ver arriba)'

# ** LOS NODOS MAESTROS DE LA TAB (2026-10-05, PLAN_TALLER 8.15): la TAB de F1
# ofrece los programas BIEN del banco de TITAN++, copiados a una tabla de
# titan-lector. Un catalogo a mano mentiria pronto; este dice NO si la tabla y
# el banco dejan de decir lo mismo. Ver toolchain/tools/maestros/maestros.py.
Guardian 'Validating the TAB of F1 offers exactly the bench of TITAN++' `
    'toolchain\tools\maestros\maestros.py' 'los nodos maestros de la TAB' `
    'maestros: maestros_gen.rs y el banco no dicen lo mismo (python toolchain/tools/maestros/maestros.py lo regenera)'

# ** LA GUIA DE ESTRATOS EN F1 (2026-10-05, PLAN_LAS_RAMAS R1): cada puerta de
# ESTRATOS que existe, con QUE hace y POR QUE, copiada del contrato del ABI a
# una tabla del TALLER. Una guia a mano mentiria pronto; este dice NO si la
# tabla y el contrato dejan de decir lo mismo, o si una puerta no se explica.
Guardian 'Validating the guide of ESTRATOS says what the contract says' `
    'toolchain\tools\estratos-guia\guia.py' 'la guia de ESTRATOS' `
    'estratos-guia: guia_estratos_gen.rs y el contrato no dicen lo mismo (python toolchain/tools/estratos-guia/guia.py lo regenera)'

# ---------------------------------------------------------------------------
# ** EL QUINTO GUARDIAN: LAS CITAS A DOCUMENTOS (2026-08-17).
#
# El arbol cita documentos desde el kernel, desde los `Cargo.toml`, desde este
# mismo fichero y desde los ejemplos de C: casi cuatrocientas veces. Nada lo
# comprobaba, y un puntero roto no falla -- manda al lector a la nada, y el
# lector concluye que el documento nunca se escribio.
#
# El dia que se escribio el guardian encontro catorce. Una de ellas apuntaba a
# AVANCES.md dentro de docs/, cuando ese fichero vive en la raiz, y **no habia
# resuelto nunca**: estaba en un documento cuyo trabajo entero es mandar al
# lector a otro sitio.
#
# ** Y el ejemplo de arriba va SIN backticks a proposito: este guardian no sabe
# distinguir una cita de la CITA DE UNA CITA ROTA, y tiene razon -- si el
# ejemplo tiene forma de ruta, es una ruta. Se cazo a si mismo en este
# comentario el dia que se escribio.
#
# ** Por que va aqui y no en un banco de pruebas: los documentos se mueven con
# `git mv` y las citas no se mueven con ellos. Eso pasa mientras se trabaja, no
# en el despliegue -- y `-BuildOnly` es lo que se corre veinte veces al dia.
# Es la leccion que ya dejo escrita el guardian del `.h`: el que se corre a mano
# no protege igual que el que se corre solo.
#
# El trato lo pone `Guardian`, arriba, y es el mismo para los tres.
# ** WAIT: un derecho que no se puede ejercer no es un derecho. Cada `grant`
# con RIGHT_WAIT tiene su brazo en `wait()`, o el build para. Nacio el 21-09
# de `KIND_ARCHIVO`, que prometia dormir sobre un mecanismo que no existia.
Guardian 'Validating every RIGHT_WAIT has an arm in wait()' `
    'toolchain\tools\esperable\esperable.py' 'los esperables' `
    'esperable: hay un KIND_ con RIGHT_WAIT que wait() no sabe esperar (ver arriba)'
Guardian 'Validating document citations resolve' `
    'toolchain\tools\enlaces\enlaces.py' 'las citas' `
    'citas: hay documentos citados que no existen (ver arriba)'
# ** L6a: TRINQUETE, no muro -- juzga el delta contra `LINEA_BASE.txt`. El por
# que esta entero en la cabecera de `censo_modular.py`, y ahi solo hay uno.
Guardian 'Validating L6a: no new module over the line' `
    'toolchain\tools\censo-modular\censo_modular.py' 'L6a' `
    'L6a: un modulo nuevo pasa de las 1.000 lineas, o uno de la linea base crecio'
# ** L8: LAS CAPAS. Muro entre crates (Ring 3 no enlaza Ring 0, nadie enlaza el
# nucleo, un puro no sabe de nadie) y trinquete dentro de los binarios (una
# pareja de subsistemas que se importan en los dos sentidos no puede ser NUEVA).
# El por que entero, en la cabecera de `capas.py` y en L8.
Guardian 'Validating L8: dependencies go down the layers' `
    'toolchain\tools\capas\capas.py' 'L8' `
    'L8: una dependencia sube de capa, o hay un nudo nuevo entre subsistemas'

# ** EL AMBITO de un commit, y SOLO el ambito. Trinquete como el de L6a, y no
# mira la prosa: el por que entero esta en la cabecera de ambitos.py.
# ** LAS CASILLAS DE LOS PLANES, que no las contaba nadie. El 24-08 se
# recontaron a mano y OCHO de cincuenta y cuatro estaban mal -- cinco escalones
# de MAQUETA figuraban sin hacer con su crate hecho y su banco verde. El por que
# entero, y por que el primer intento de este guardian no cazo ninguna, esta en
# la cabecera de casillas.py -- la leccion no es la que se fue a buscar.
Guardian 'Validating plan checkboxes are verifiable' `
    'toolchain\tools\casillas\casillas.py' 'las casillas' `
    'casillas: una casilla no se puede comprobar (ver arriba)'

# ** Y EL INDICE DE LO QUE FALTA, que es el hermano del de arriba (2026-09-10).
#
# `casillas` comprueba que una casilla DIGA DONDE MIRAR. Este comprueba que el
# mapa de todas ellas --docs/plan/ABIERTO.md-- siga diciendo lo mismo que los
# veintiseis planes. Sin el, saber que queda pendiente obliga a abrir
# veintiseis ficheros y contar a mano, asi que no lo hace nadie y la respuesta
# sale de la memoria en vez de salir del arbol.
#
# [!] Ademas NOMBRA los planes que estan en `plan/` sin ni una casilla: un
# fichero que no dice que falta no contesta la pregunta de su carpeta. El
# porque entero en la cabecera de planes.py.
Guardian 'Validating the open-work index matches the plans' `
    'toolchain\tools\planes\planes.py' 'el indice de lo que falta' `
    'planes: el indice y los planes no dicen lo mismo (se arregla con --apply)'
# ** QUIEN ESCRIBE CADA `static mut` DEL USB (2026-09-18, PLAN_EL_BUS_APARTE
# A0.3). El bus quiere su propio nucleo, y la primera pregunta --cuales de
# sus 57 estaticos tocan los dos lados-- no tenia respuesta sin leer el codigo
# entero. Ahora cada uno lo dice en su linea, uno nuevo sin decirlo para el
# build, y `ambos` es un trinquete. Ver toolchain/tools/escritores/escritores.py
Guardian 'Validating every USB static says who writes it' `
    'toolchain\tools\escritores\escritores.py' 'los escritores del USB' `
    'escritores: un static mut del USB no dice quien lo escribe, o `ambos` subio (ver arriba)'
# ** LA 3060 (25-09): el dia que funciono entera, el propietario pidio que los
# guardianes la PROTEJAN antes de optimizarla. La puerta de sus 65 ordenes
# pide la autoridad MAQUINA (una app con la pantalla prestada no manda en
# ella), sus registros solo se tocan en dev/gpu*, sus ordenes y motivos valen
# lo mismo en kernel, ABI y userland, y sus esperas girando son un trinquete.
# Ver toolchain/tools/la-3060/la_3060.py
Guardian 'Validating the 3060 stays fenced' `
    'toolchain\tools\la-3060\la_3060.py' 'la puerta y el cerco de la 3060' `
    'la-3060: la 3060 se aflojo (ver arriba)'
# ** EL DISCO PERSONAL (29-09, N1a): el kernel lee el NTFS del OTRO SSD SATA
# (Personal D:, donde esta Cyberpunk 2077) y NUNCA lo escribe; C: (el NVMe)
# ni se mira. El propietario: "pon guardianes tambien". Dos cerrojos y un
# solo propietario. Ver toolchain/tools/ajeno/ajeno.py
Guardian 'Validating the foreign disk stays read-only' `
    'toolchain\tools\ajeno\ajeno.py' 'el disco PERSONAL solo lectura' `
    'ajeno: el disco PERSONAL o C: quedaron al alcance de una escritura (ver arriba)'
Guardian 'Validating compiler warnings do not grow' `
    'toolchain\tools\avisos\avisos.py' 'los avisos del compilador' `
    'avisos: los avisos del compilador SUBIERON (ver arriba)'
# ** EL EJE PROPIO DEL COMPILADOR DE C: donde nace un fallo y DONDE APARECE.
#
# Un compilador falla distinto a un kernel: en Ring 0 el fallo se paga donde
# esta, y aqui se paga LEJOS -- todo en verde, y el sintoma dentro de DOOM tres
# semanas despues. Este guardian muestra la lista de los `DENTRO`, que es el mapa
# de los sitios donde el banco NO protege. Ver toolchain/tools/fases/fases.py
Guardian 'Validating BMO C declares where its failures appear' `
    'toolchain/tools/fases/fases.py' 'las fases de BMO C' `
    'fases: un fichero de BMO C perdio su [fase] o inventa un valor (ver arriba)'
Guardian 'Validating commit scopes' `
    'toolchain\tools\ambitos\ambitos.py' 'los ambitos de los commits' `
    'ambitos: un commit usa un ambito que no esta en AMBITOS.txt (ver arriba)'

# ** EL CENSO DEL NEUTRO contra el codigo. `NEUTRO/CENSO.txt` se escribe a mano
# y los marcos se etiquetan en otro sitio: dos listas de lo mismo que pueden
# separarse sin que nadie avise -- el `[riesgo] ESPEJO` de esta casa, que ya se
# pago con las constantes del ABI.
#
# [!] Y NO comprueba el censo contra la MAQUINA: aqui no hay bus PCI. Esa mitad
# es el `sincodigo` del portero y hay que arrancar para verla. El porque entero
# esta en la cabecera de censo_neutro.py.
Guardian 'Validating NEUTRO census matches the code' `
    'toolchain\tools\censo-neutro\censo_neutro.py' 'el censo del neutro' `
    'censo-neutro: el censo y el codigo no dicen lo mismo (ver arriba)'

# ** EL PERFIL DE LA PLACA contra los rodeos. Lo que se sabe de esta A320M
# vivia repartido en tres capas --el sobre del traspaso, la etapa s1 y el
# kernel-- y ninguna sabia de las otras. El porque entero en PERFIL/PLACA/README.md.
#
# [!] Y NO comprueba que la placa puesta sea esta: eso lo sabria el firmware.
# El perfil se DECLARA, no se detecta.
Guardian 'Validating board profile matches its workarounds' `
    'toolchain\tools\perfil-placa\perfil_placa.py' 'el perfil de la placa' `
    'perfil-placa: el perfil y los rodeos no dicen lo mismo (ver arriba)'

# ** Y QUE TODO PERFIL DIGA A QUIEN EXPONE. Un perfil que expone a un fichero
# borrado no avisa de nada: describe una maquina que ya no esta y suena igual
# de seguro. Idea del propietario -- 'si falla, el guardian lo frena por motivos'.
Guardian 'Validating every profile declares what it exposes' `
    'toolchain\tools\perfil\perfil.py' 'las exposiciones de los perfiles' `
    'perfil: un perfil no dice a quien expone, o expone a algo que no existe'

# ** Y EL CONTENIDO, no solo la flecha: que lo que cada perfil AFIRMA sea lo que
# el codigo dice. Incluye LOS TRES CIERRES de build/discos.ps1 -- la
# comprobacion mas importante del repo, porque ese fichero es el unico que
# puede escribir en el disco del propietario. Ver PERFIL/DISCO.txt.
Guardian 'Validating profile fields match the code' `
    'toolchain\tools\perfil-campos\perfil_campos.py' 'los campos de los perfiles' `
    'perfil-campos: un perfil afirma algo que el codigo no dice (ver arriba)'

# ** Y QUE UNA BANDERA DECLARADA LLEGUE AL OTRO LADO. `desplegar.ps1` declaraba
# `-Si` y no se lo pasaba a `bmo.ps1`: se aceptaba sin protestar y el despliegue
# preguntaba igual. PowerShell no se queja de un `param` que no se use, asi que
# el hueco solo se ve cuando algo no pasa.
Guardian 'Validating no flag is dropped in the handoff' `
    'toolchain\tools\relevo\relevo.py' 'el relevo de las banderas' `
    'relevo: una bandera se declara y no viaja -- se acepta y no hace nada'
