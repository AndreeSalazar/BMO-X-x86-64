//! Las pruebas de la mezcla, sobre IMAGENES de verdad (un fichero temporal):
//! la regla de `ESTRATOS.md` -- primero en imagenes, despues en F:.

use super::*;
use std::fs::{self, File, OpenOptions};
use std::time::{SystemTime, UNIX_EPOCH};

const BLOQUES: u64 = 4096;

fn id() -> [u8; 32] {
    es::disk_id(b"TEST MEZCLA", b"SERIE-MEZCLA", BLOQUES)
}

struct Imagen {
    ruta: std::path::PathBuf,
    disco: File,
    generacion: u64,
}

impl Drop for Imagen {
    fn drop(&mut self) {
        // `BMO_MEZCLA_GUARDA=<carpeta>` deja una copia de cada imagen, para
        // pasarle `estratos-fmt <imagen> --verificar` (el verificador de
        // siempre, no uno de esta prueba).
        if let Some(dir) = std::env::var_os("BMO_MEZCLA_GUARDA") {
            let _ = fs::copy(&self.ruta, std::path::Path::new(&dir).join(self.ruta.file_name().unwrap()));
        }
        let _ = fs::remove_file(&self.ruta);
    }
}

/// Un volumen recien formateado: una raiz vacia y su estrato.
fn imagen(nombre: &str) -> (Imagen, BlockPtr) {
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let ruta = std::env::temp_dir().join(format!("bmo-mezcla-{nombre}-{}-{nonce}.img", std::process::id()));
    let mut disco = OpenOptions::new().read(true).write(true).create_new(true).open(&ruta).unwrap();
    disco.set_len(BLOQUES * BLOQUE as u64).unwrap();
    let raiz = escribir_objeto(&mut disco, 2, &es::escritura::nodo_de_directorio_vacio()).unwrap();
    let e = Estrato::new(raiz, BlockPtr::NULO, 0, Autor::Herramienta, "formato");
    let ep = escribir_objeto(&mut disco, 3, &e.encode()).unwrap();
    let mut sb = Superblock::new(id(), BLOQUES);
    sb.generation = 1;
    sb.log_head = 4;
    sb.estrato = ep;
    for lba in [es::SUPER_A_BLOCK, es::SUPER_B_BLOCK] {
        disco.seek(SeekFrom::Start(lba * BLOQUE as u64)).unwrap();
        disco.write_all(&sb.encode()).unwrap();
    }
    disco.sync_all().unwrap();
    (Imagen { ruta, disco, generacion: 1 }, ep)
}

fn carpeta(ficheros: &[(&[u8], &[u8])]) -> Carpeta {
    let mut c = Carpeta::default();
    for (r, d) in ficheros {
        c.poner(r, Hijo::Contenido(d.to_vec())).unwrap();
    }
    c
}

/// Publica una version con `padre` elegido (no el de ahora): asi se hacen ramas.
fn version(img: &mut Imagen, padre: BlockPtr, ficheros: &[(&[u8], &[u8])]) -> BlockPtr {
    let (g, ep, _, _) = publicar(&mut img.disco, id(), img.generacion, &Hijo::Carpeta(carpeta(ficheros)), |raiz, _| {
        Estrato::new(raiz, padre, 0, Autor::Herramienta, "")
    })
    .unwrap();
    img.generacion = g;
    ep
}

/// Lo que dice cada fichero del arbol de un estrato: (ruta, contenido).
fn contenido(img: &mut Imagen, estrato: &BlockPtr) -> Vec<(String, Vec<u8>)> {
    let raiz = leer_estrato(&mut img.disco, estrato).unwrap().raiz;
    let mut v: Vec<(String, Vec<u8>)> = aplanar(&mut img.disco, &raiz)
        .unwrap()
        .into_iter()
        .map(|h| {
            let n = leer_nodo(&mut img.disco, &h.nodo).unwrap();
            let d = leer_flujo(&mut img.disco, n.attr(":datos").unwrap()).unwrap();
            (String::from_utf8_lossy(&h.ruta).into_owned(), d)
        })
        .collect();
    v.sort();
    v
}

fn punta(img: &mut Imagen) -> BlockPtr {
    abrir(&mut img.disco, id(), img.generacion).unwrap().0.estrato
}

fn s(r: &str, d: &str) -> (String, Vec<u8>) {
    (r.to_string(), d.as_bytes().to_vec())
}

#[test]
fn mezcla_dos_ramas_por_nodos_con_un_choque_elegido_y_dos_padres() {
    let (mut img, formato) = imagen("ramas");
    let base = version(&mut img, formato, &[
        (b"mundo/a.txt", b"a de la base"),
        (b"mundo/b.txt", b"b de la base"),
        (b"leeme", b"igual en todas"),
        (b"choque.txt", b"base"),
    ]);
    // La rama que entra: cambia b y el choque.
    let rama = version(&mut img, base, &[
        (b"mundo/a.txt", b"a de la base"),
        (b"mundo/b.txt", b"b de la RAMA"),
        (b"leeme", b"igual en todas"),
        (b"choque.txt", b"lo de la rama"),
    ]);
    // La de ahora: cambia a, agrega uno, y tambien el choque.
    let ahora = version(&mut img, base, &[
        (b"mundo/a.txt", b"a de AHORA"),
        (b"mundo/b.txt", b"b de la base"),
        (b"leeme", b"igual en todas"),
        (b"choque.txt", b"lo de ahora"),
        (b"nuevo.txt", b"solo ahora"),
    ]);
    assert_eq!(punta(&mut img), ahora);
    let rama_antes = contenido(&mut img, &rama);

    let mut choques = Vec::new();
    let r = mezclar(&mut img.disco, id(), img.generacion, &rama, "mezcla de la rama", &mut |c| {
        choques.push(String::from_utf8_lossy(&c.ruta).into_owned());
        Eleccion::B
    })
    .unwrap();
    img.generacion = r.generacion;

    assert_eq!(choques, ["choque.txt"], "un choque, y una persona eligio");
    assert_eq!(contenido(&mut img, &r.estrato), vec![
        s("choque.txt", "lo de la rama"),
        s("leeme", "igual en todas"),
        s("mundo/a.txt", "a de AHORA"),
        s("mundo/b.txt", "b de la RAMA"),
        s("nuevo.txt", "solo ahora"),
    ]);
    // DOS padres: la punta de antes y la rama.
    let e = leer_estrato(&mut img.disco, &r.estrato).unwrap();
    assert_eq!(e.padre, ahora);
    assert!(e.segundo.unwrap().es(&rama));
    assert_eq!(punta(&mut img), r.estrato);
    // Nada se pierde: la rama y la de antes siguen enteras.
    assert_eq!(contenido(&mut img, &rama), rama_antes);
    assert_eq!(contenido(&mut img, &ahora).len(), 5);
}

#[test]
fn los_ficheros_no_se_copian_la_mezcla_apunta_a_los_mismos_nodos() {
    // D4 (a): nacen COMPARTIENDO. Solo se escriben carpetas y el estrato.
    let (mut img, formato) = imagen("compartir");
    let base = version(&mut img, formato, &[(b"x", b"1"), (b"y", b"2")]);
    let rama = version(&mut img, base, &[(b"x", b"1"), (b"y", b"2 de la rama")]);
    let _ahora = version(&mut img, base, &[(b"x", b"1 de ahora"), (b"y", b"2")]);
    let r = mezclar(&mut img.disco, id(), img.generacion, &rama, "m", &mut |_| Eleccion::A).unwrap();
    let raiz_rama = leer_estrato(&mut img.disco, &rama).unwrap().raiz;
    let y_rama = aplanar(&mut img.disco, &raiz_rama).unwrap().into_iter().find(|h| h.ruta == b"y").unwrap();
    let y_mezcla = aplanar(&mut img.disco, &r.raiz).unwrap().into_iter().find(|h| h.ruta == b"y").unwrap();
    assert_eq!(y_mezcla.nodo, y_rama.nodo, "el MISMO nodo, en el mismo sitio: no se copio un byte");
    // Una carpeta raiz (lista + nodo) y el estrato: tres bloques.
    assert_eq!(r.bloques_nuevos, 3);
}

#[test]
fn mezclar_una_rama_que_ya_esta_dentro_no_escribe_nada() {
    let (mut img, formato) = imagen("dentro");
    let base = version(&mut img, formato, &[(b"x", b"1")]);
    let _ahora = version(&mut img, base, &[(b"x", b"2")]);
    let antes = fs::read(&img.ruta).unwrap();
    let e = mezclar(&mut img.disco, id(), img.generacion, &base, "m", &mut |_| Eleccion::A).unwrap_err();
    assert!(e.contains("nada que mezclar"), "{e}");
    assert_eq!(fs::read(&img.ruta).unwrap(), antes, "ni un byte cambio");
}

#[test]
fn la_base_se_encuentra_tambien_por_el_segundo_padre() {
    // Mezclar dos veces la misma rama: la segunda vez, la base es la rama de
    // la primera mezcla, alcanzada por el SEGUNDO padre.
    let (mut img, formato) = imagen("segundo");
    let base = version(&mut img, formato, &[(b"x", b"1"), (b"y", b"1")]);
    let rama = version(&mut img, base, &[(b"x", b"1"), (b"y", b"rama 1")]);
    let _ahora = version(&mut img, base, &[(b"x", b"ahora"), (b"y", b"1")]);
    let m1 = mezclar(&mut img.disco, id(), img.generacion, &rama, "m1", &mut |_| Eleccion::A).unwrap();
    img.generacion = m1.generacion;
    // La rama sigue: un cambio mas, con la rama de antes de padre.
    let rama2 = version(&mut img, rama, &[(b"x", b"1"), (b"y", b"rama 2")]);
    // La punta es ahora rama2 (version publica la punta); se vuelve a la mezcla
    // publicando una version igual a m1 con m1 de padre.
    let m1_arbol = contenido(&mut img, &m1.estrato);
    let ficheros: Vec<(Vec<u8>, Vec<u8>)> = m1_arbol.iter().map(|(r, d)| (r.as_bytes().to_vec(), d.clone())).collect();
    let refs: Vec<(&[u8], &[u8])> = ficheros.iter().map(|(r, d)| (&r[..], &d[..])).collect();
    let _vuelta = version(&mut img, m1.estrato, &refs);
    let mut hubo_choque = false;
    let m2 = mezclar(&mut img.disco, id(), img.generacion, &rama2, "m2", &mut |_| {
        hubo_choque = true;
        Eleccion::A
    })
    .unwrap();
    // Con la base bien encontrada (la rama de m1), solo B cambio `y`: sin choque.
    assert!(!hubo_choque, "la base por el segundo padre evita un choque falso");
    let c = contenido(&mut img, &m2.estrato);
    assert!(c.contains(&s("y", "rama 2")) && c.contains(&s("x", "ahora")), "{c:?}");
}

#[test]
fn un_nombre_latin1_sobrevive_a_la_mezcla() {
    let (mut img, formato) = imagen("latin1");
    let anio: &[u8] = b"a\xF1o.txt";
    let base = version(&mut img, formato, &[(anio, b"1"), (b"z", b"1")]);
    let rama = version(&mut img, base, &[(anio, b"1"), (b"z", b"2")]);
    let _ahora = version(&mut img, base, &[(anio, b"de ahora"), (b"z", b"1")]);
    let r = mezclar(&mut img.disco, id(), img.generacion, &rama, "m", &mut |_| Eleccion::A).unwrap();
    let hojas = aplanar(&mut img.disco, &r.raiz).unwrap();
    assert!(hojas.iter().any(|h| h.ruta == anio), "el nombre en sus bytes, entero");
}

/// Un volumen al azar: (ruta, contenido), sin repetir rutas.
type Arbol = Vec<(String, String)>;

fn azar(semilla: u64) -> impl FnMut(u64) -> u64 {
    let mut s = semilla.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    move |n| {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        s % n
    }
}

const CARPETAS: [&str; 5] = ["", "d0/", "d1/", "d0/e/", "d1/f/g/"];

fn editar(base: &Arbol, lado: &str, r: &mut impl FnMut(u64) -> u64) -> Arbol {
    let mut v = base.clone();
    for _ in 0..1 + r(4) {
        match r(5) {
            // cambiar uno; a veces igual que lo cambiaria el otro lado
            0 | 1 if !v.is_empty() => {
                let i = r(v.len() as u64) as usize;
                v[i].1 = if r(3) == 0 { format!("igual {}", v[i].0) } else { format!("{lado} {}", r(1000)) };
            }
            2 if !v.is_empty() => {
                let i = r(v.len() as u64) as usize;
                v.remove(i);
            }
            _ => {
                let ruta = format!("{}n{}", CARPETAS[r(5) as usize], r(6));
                if !v.iter().any(|(p, _)| *p == ruta) {
                    v.push((ruta.clone(), if r(3) == 0 { format!("igual {ruta}") } else { format!("{lado} nuevo {}", r(1000)) }));
                }
            }
        }
    }
    v
}

fn publicar_arbol(img: &mut Imagen, padre: BlockPtr, a: &Arbol) -> BlockPtr {
    let f: Vec<(Vec<u8>, Vec<u8>)> = a.iter().map(|(p, d)| (p.as_bytes().to_vec(), d.as_bytes().to_vec())).collect();
    let refs: Vec<(&[u8], &[u8])> = f.iter().map(|(p, d)| (&p[..], &d[..])).collect();
    version(img, padre, &refs)
}

/// ** EL ORACULO: la mezcla POR CARPETAS (la de verdad) y la PLANA (fichero a
/// fichero) deciden lo mismo, sobre arboles al azar con cambios en los dos
/// lados -- y despues la mezcla se publica entera y se relee.
#[test]
fn por_carpetas_y_plana_deciden_lo_mismo_al_azar() {
    let semillas = std::env::var("E1_AZAR_MEZCLA").ok().and_then(|v| v.parse().ok()).unwrap_or(60u64);
    let mut con_choque = 0;
    for semilla in 1..=semillas {
        let mut r = azar(semilla);
        let mut base: Arbol = Vec::new();
        for i in 0..4 + r(8) {
            let ruta = format!("{}f{i}", CARPETAS[r(5) as usize]);
            base.push((ruta, format!("base {i}")));
        }
        let (a, b) = (editar(&base, "A", &mut r), editar(&base, "B", &mut r));
        let (mut img, formato) = imagen(&format!("azar{semilla}"));
        let pb = publicar_arbol(&mut img, formato, &base);
        let px = publicar_arbol(&mut img, pb, &b);
        let pa = publicar_arbol(&mut img, pb, &a);
        let raiz = |img: &mut Imagen, p: &BlockPtr| leer_estrato(&mut img.disco, p).unwrap().raiz;
        let (rb, ra, rx) = (raiz(&mut img, &pb), raiz(&mut img, &pa), raiz(&mut img, &px));

        let mut choques = 0;
        // Las tres respuestas que puede dar una persona, y las dos maneras de
        // decidir tienen que coincidir con cada una.
        for eleccion in [Eleccion::A, Eleccion::B, Eleccion::Quitar] {
            let (arbol, _) = decide::por_arbol(&mut img.disco, rb, ra, rx, &mut |_| {
                choques += 1;
                eleccion
            })
            .unwrap();
            let plano = decide::plano(&mut img.disco, rb, ra, rx, &mut |_| eleccion).unwrap();
            let mut x = decide::hojas_de(&mut img.disco, &arbol).unwrap();
            let mut y = decide::hojas_de(&mut img.disco, &Hijo::Carpeta(plano)).unwrap();
            x.sort();
            y.sort();
            assert_eq!(x, y, "semilla {semilla}, {eleccion:?}: base {base:?}\nA {a:?}\nB {b:?}");
        }
        if choques > 0 {
            con_choque += 1;
        }
        // Y entera: publicada, con sus dos padres, y releida.
        mezclar(&mut img.disco, id(), img.generacion, &px, "azar", &mut |_| Eleccion::A).unwrap();
    }
    assert!(con_choque > 0, "el azar tiene que traer choques, si no no prueba nada");
}

fn muchos(n: usize, cambia: &str, cuales: &[usize]) -> Vec<(Vec<u8>, Vec<u8>)> {
    (0..n)
        .map(|i| {
            let d = if cuales.contains(&i) { format!("{cambia} {i}") } else { format!("base {i}") };
            (format!("grande/f{i:02}").into_bytes(), d.into_bytes())
        })
        .collect()
}

fn refs(v: &[(Vec<u8>, Vec<u8>)]) -> Vec<(&[u8], &[u8])> {
    v.iter().map(|(p, d)| (&p[..], &d[..])).collect()
}

#[test]
fn una_carpeta_de_mas_de_un_bloque_se_mezcla_entera() {
    // 50 entradas: la lista ya no cabe en un bloque (36), y lleva indice.
    let (mut img, formato) = imagen("grande");
    let base = version(&mut img, formato, &refs(&muchos(50, "", &[])));
    let rama = version(&mut img, base, &refs(&muchos(50, "B", &[3, 40, 49])));
    let _ahora = version(&mut img, base, &refs(&muchos(50, "A", &[0, 20])));
    let r = mezclar(&mut img.disco, id(), img.generacion, &rama, "grande", &mut |_| Eleccion::A).unwrap();
    let c = contenido(&mut img, &r.estrato);
    assert_eq!(c.len(), 50);
    for (i, quien) in [(0, "A"), (20, "A"), (3, "B"), (40, "B"), (49, "B"), (7, "base")] {
        assert!(c.contains(&s(&format!("grande/f{i:02}"), &format!("{quien} {i}"))), "f{i}: {c:?}");
    }
}

#[test]
fn una_carpeta_de_mas_de_64_se_dice_y_no_se_escribe_nada() {
    let (mut img, formato) = imagen("tope");
    let base = version(&mut img, formato, &refs(&muchos(70, "", &[])));
    let rama = version(&mut img, base, &refs(&muchos(70, "B", &[1])));
    let _ahora = version(&mut img, base, &refs(&muchos(70, "A", &[2])));
    let antes = fs::read(&img.ruta).unwrap();
    let e = mezclar(&mut img.disco, id(), img.generacion, &rama, "tope", &mut |_| Eleccion::A).unwrap_err();
    assert!(e.contains("64 entradas"), "{e}");
    assert_eq!(fs::read(&img.ruta).unwrap(), antes, "ni un byte cambio");
}

fn version_de(img: &mut Imagen, lba: u64) -> u32 {
    let b = leer_bloque(&mut img.disco, lba).unwrap();
    u32::from_le_bytes([b[8], b[9], b[10], b[11]])
}

/// ** EL CASO DE D5, que con `volver` fallaba: principal avanza, se cambia a
/// "pruebas" (que salio de X), pruebas avanza, y principal se MEZCLA en
/// pruebas. Con la tabla de ramas nadie es antepasado de nadie, y la mezcla
/// trae lo de las dos.
#[test]
fn con_la_tabla_de_ramas_se_mezcla_la_rama_que_se_dejo() {
    let (mut img, formato) = imagen("d5");
    let x = version(&mut img, formato, &[(b"a", b"a de X"), (b"b", b"b de X")]);
    assert_eq!((version_de(&mut img, 0), version_de(&mut img, 1)), (1, 1), "sin ramas: v1");
    img.generacion = ramas::crear_rama(&mut img.disco, id(), img.generacion, b"pruebas", None, b"principal").unwrap();
    assert_eq!((version_de(&mut img, 0), version_de(&mut img, 1)), (2, 2), "la subida escribe LAS DOS copias");

    let t1 = version(&mut img, x, &[(b"a", b"a de principal"), (b"b", b"b de X")]);
    img.generacion = ramas::cambiar_rama(&mut img.disco, id(), img.generacion, b"pruebas").unwrap();
    assert_eq!(punta(&mut img), x, "el superbloque sigue la punta de pruebas");
    assert_eq!(ramas::punta_de(&mut img.disco, id(), img.generacion, b"principal").unwrap(), t1);
    let _t3 = version(&mut img, x, &[(b"a", b"a de X"), (b"b", b"b de pruebas")]);

    let principal = ramas::punta_de(&mut img.disco, id(), img.generacion, b"principal").unwrap();
    let r = mezclar(&mut img.disco, id(), img.generacion, &principal, "principal en pruebas", &mut |_| Eleccion::A).unwrap();
    img.generacion = r.generacion;
    assert_eq!(contenido(&mut img, &r.estrato), vec![s("a", "a de principal"), s("b", "b de pruebas")]);
    // La tabla sobrevive a los commits de despues (el de la mezcla, aqui).
    let t = ramas::leer_ramas(&mut img.disco, id(), img.generacion).unwrap().unwrap();
    assert_eq!((t.cuantas(), t.actual()), (2, &b"pruebas"[..]));
    assert_eq!((version_de(&mut img, 0), version_de(&mut img, 1)), (2, 2));
}

/// Y por que hacia falta: con `volver` (un estrato con la raiz de X y la punta
/// de ahora de padre), la rama que se deja es ANTEPASADA y no se mezcla.
#[test]
fn sin_tabla_volver_hace_antepasada_a_la_rama_que_se_deja() {
    let (mut img, formato) = imagen("d5-sin");
    let x = version(&mut img, formato, &[(b"a", b"a de X")]);
    let t1 = version(&mut img, x, &[(b"a", b"a de principal")]);
    let raiz_x = leer_estrato(&mut img.disco, &x).unwrap().raiz;
    let (g, _, _, _) = publicar(&mut img.disco, id(), img.generacion, &Hijo::Nodo(raiz_x), |raiz, padre| {
        Estrato::new(raiz, padre, 0, Autor::Herramienta, "")
    })
    .unwrap();
    img.generacion = g;
    let e = mezclar(&mut img.disco, id(), img.generacion, &t1, "m", &mut |_| Eleccion::A).unwrap_err();
    assert!(e.contains("nada que mezclar"), "{e}");
}
