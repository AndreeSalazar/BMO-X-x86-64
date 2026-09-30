"""enes_caidas -- el diccionario CERRADO de la ene caida, para ascii_sweep.py.

Vivia dentro de `ascii_sweep.py` y se mudo aqui el 2026-09-30, texto movido y
no reescrito: son DATOS, y al agregar `peldano` el fichero paso de las 1.000
lineas de L6a. El porque entero sigue abajo, tal cual estaba.
"""

# == *** LA ENE CON TILDE QUE, AL CAERSE, DICE OTRA COSA ====================
#
# Esta casa escribe en ASCII y la regla es **quitar la tilde de la ene**:
# `tamano`, `pequeno`, `senal`, `ninguno`. Funciona porque lo que queda no es
# ninguna palabra, asi que se lee como lo que era.
#
# ** Con DOS no funciona, y el dueno lo dijo el 2026-09-09 riendose: una de
# ellas ES otra palabra, y no es la que se queria decir.
#
# *** Y NO todas las que acaban parecido son sospechosas. Se comprobaron una a
# una antes de escribir esto:
#
#     campana   la CAMPANA del AHCI (`doorbell`). Correcta, se queda
#     sana      "una placa sana". Correcta, se queda
#
# Por eso esto es un diccionario CERRADO y no un patron: un patron habria
# marcado esas dos, y un guardian que se equivoca se apaga en una semana.
#
# [!] Lo que SACRIFICA (L3): hay que anadir a mano la siguiente que aparezca.
# Es el mismo precio que `COSTES` y `RIESGOS` del contrato, y por el mismo
# motivo: un vocabulario cerrado se puede comprobar; uno abierto, no.
# ** Y LA EXCEPCION, que la trajo un test roto el mismo dia.
#
# `dynobj/texto.rs` existe para demostrar que la cabecera cuenta BYTES y no
# caracteres, y su ejemplo es el par: la palabra en ASCII (4 bytes) contra la
# misma con la ene con tilde (4 caracteres, CINCO bytes). El barrido convirtio
# la primera en `anios` --y la puso en 5 bytes-- asi que **el test que explica
# la ene con tilde murio por quitarle la ene con tilde**.
#
# *** Ahi la forma ASCII es CORRECTA: es la mitad de una pareja a proposito. Se
# exime la LINEA y no el fichero, y con una marca que obliga a decirlo.
#
# [!] Eximir el fichero entero habria dejado pasar las que SI son un fallo en
# el mismo sitio. Es la misma forma que R14 con sus sondas: se anota, no se
# apaga.
MARCA_ADREDE = "ene-caida-adrede"

# == *** Y DESDE EL 2026-09-21 (noche) LA REGLA ES ESTRICTA ==================
#
# El dueno, literal: *"que sea estricto: ni con ene con tilde, ni con tilde,
# ni "duenno" o "dueno" porque no tiene sentido, y ano eso es estupido; es
# spanglish pero vamos a madurar eso"*. La regla del 2026-08-20 (INTI) ya lo
# decia para una palabra: **perder una TILDE no es perder una letra; perder
# la ene SI**, y una palabra a la que le falta una letra no es ni castellano
# ni ingles: es una palabra rota. `tamano` se fue a `medida` entonces. Hoy se
# van TODAS, con el mismo remedio: **la palabra que sobrevive entera** (un
# sinonimo castellano, o el ingles cuando es lo natural en un comentario).
#
# ** Sigue siendo un diccionario CERRADO, por lo mismo de antes: un patron
# marcaria `campana` (la CAMPANA del AHCI, correcta), `pena` ("vale la
# pena", correcta), `una`, `mono`, `cuna`. Cada entrada de aqui se comprobo
# contra lo que el repositorio dice de verdad; las ambiguas NO estan, y se
# miran a ojo cuando se escriben.
#
# Cada entrada: (patron, remedio). El patron casa la palabra ENTERA en
# minusculas; el remedio puede usar los grupos. `reponer_enes` conserva la
# forma --minuscula, Capital, MAYUSCULA-- de lo que habia.
ENES_CAIDAS = [
    # dueno, duena, duenos, duenas
    (r"duen(o|a|os|as)", r"propietari\1"),
    # tamano(s): la decision del 2026-08-20 (INTI), extendida
    (r"tamano(s?)", r"medida\1"),
    # senal, senales -> signal(s); senalar y su conjugacion -> marcar
    (r"senal(es)?", r"signal\1"),
    (r"senal(ar|a|an|o|ado|ada|ados|adas|aba|aban|ando|amos|aron|ara|aran)", r"marc\1"),
    (r"senal(e|en)", r"marqu\1"),
    # pequeno/a/os/as, pequenito...
    (r"pequen(o|a|os|as|ito|ita|itos|itas|isimo|isima)", r"chic\1"),
    # nino/a/os/as
    (r"nin(o|a|os|as)", r"cri\1"),
    # sueno(s), y el ano que dice otra cosa
    (r"suen(o|os)", r"repos\1"),
    (r"ano", "anualidad"),
    (r"anos", "anualidades"),
    # diseno(s) el nombre; disenar y su conjugacion
    (r"diseno", "esquema"),
    (r"disenos", "esquemas"),
    (r"disen(ar|a|an|ado|ada|ados|adas|aba|aban|ando|amos|aron|ara|aran)", r"traz\1"),
    (r"disen(e|en)", r"trac\1"),
    (r"disenador(es)?", r"trazador\1"),
    # companero/a/os/as -> colega(s)
    (r"companer(o|a)", "colega"),
    (r"companer(os|as)", "colegas"),
    # contrasena(s) -> clave(s)
    (r"contrasena(s?)", r"clave\1"),
    # manana: el adverbio y el nombre
    (r"manana(s?)", "luego"),
    # ensenar y su conjugacion -> mostrar; ensenanza -> leccion
    (r"ensen(a|an|e|en)", r"muestr\1"),
    (r"ensen(ar|o|ado|ada|ados|adas|aba|aban|ando|amos|aron|ara|aran|aria|arian)", r"mostr\1"),
    (r"ensenanza(s?)", r"leccion\1"),
    (r"ensenanzas", "lecciones"),
    # anadir y su conjugacion -> agregar
    (r"anadir", "agregar"),
    (r"anade", "agrega"),
    (r"anaden", "agregan"),
    (r"anado", "agrego"),
    (r"anadi", "agregue"),
    (r"anadido", "agregado"),
    (r"anadida", "agregada"),
    (r"anadidos", "agregados"),
    (r"anadidas", "agregadas"),
    (r"anadiendo", "agregando"),
    (r"anadimos", "agregamos"),
    (r"anadia", "agregaba"),
    (r"anadian", "agregaban"),
    (r"anadira", "agregara"),
    (r"anadiran", "agregaran"),
    (r"anadiria", "agregaria"),
    (r"anadan", "agreguen"),
    (r"anadidura", "propina"),
    # espanol(a/es/as) -> castellano
    (r"espanol", "castellano"),
    (r"espanola", "castellana"),
    (r"espanoles", "castellanos"),
    (r"espanolas", "castellanas"),
    # dano(s) -> perjuicio; danar y su conjugacion -> perjudicar
    (r"dan(o|os)", r"perjuici\1"),
    (r"dan(ar|ado|ada|ados|adas|ando|aba|aban|aron|ara|aran)", r"perjudic\1"),
    # extrano/a -> raro/a; extraneza -> rareza
    (r"extran(o|a|os|as)", r"rar\1"),
    (r"extraneza(s?)", r"rareza\1"),
    # engano(s) -> trampa(s); enganar -> burlar; enganoso -> falaz
    (r"engan(o|os)", r"tramp\1"),
    (r"enganos", "trampas"),
    (r"engan(ar|a|an|ado|ada|ados|adas|ando|aba|aban|aron)", r"burl\1"),
    (r"enganoso", "falaz"),
    (r"enganosa", "falaz"),
    (r"enganosos", "falaces"),
    (r"enganosas", "falaces"),
    # pestana(s) (la de una ventana) -> solapa(s)
    (r"pestana(s?)", r"solapa\1"),
    # montana(s) -> sierra(s)
    (r"montana(s?)", r"sierra\1"),
    # peldano(s) -> nivel(es): se colo en TITAN_MAESTRO 14.14 el 2026-09-30 y
    # este diccionario no la tenia; la cazo quien escribia, no el guardian
    (r"peldano", "nivel"),
    (r"peldanos", "niveles"),
    # apano(s) -> arreglo(s)
    (r"apano(s?)", r"arreglo\1"),
    # entrana(s) -> tripa(s)
    (r"entrana(s?)", r"tripa\1"),
    # bano(s) -> aseo(s); puno(s) -> punal no: -> mano cerrada; senor -> don
    (r"bano(s?)", r"aseo\1"),
    (r"punetazo(s?)", r"golpe\1"),
    (r"senor(es)?", r"don"),
    (r"senora(s?)", r"dona"),
    (r"carino", "afecto"),
    (r"canon(es)?", None),   # ambigua a proposito: se mira a ojo, no se cambia
]
