//! **El banco de NTFS**, contra un disco HECHO POR `mkntfs` y llenado por
//! `ntfs-3g` (`prueba/disco.ntfs`; como se hizo, en `platform/drivers/storage/ntfs/prueba/COMO.md`): el
//! formato lo pone NTFS de verdad, no lo que este lector cree que es.
//!
//! Lo que tiene dentro (los contenidos salen de `patron`, como en el guion):
//!
//! ```text
//!    hola.txt                          16 B, RESIDENTE (dentro del registro)
//!    vacio.bin                         0 B
//!    Caf\u{e9} \u{f1}and\u{fa}.txt           nombre con acentos (UTF-16)
//!    bin/x64/Cyberpunk2077.exe         70.000 B, patron 1
//!    archive/pc/content/basegame_4_gamedata.archive   300.000 B, patron 2
//!    r6/scripts/                       80 nombres LARGOS: el indice no cabe
//!                                      en la raiz, va en bloques INDX
//!    frag/                             60 rellenos vivos (los impares) y 60
//!                                      BORRADOS (los pares), y
//!    frag/partido.bin                  200.000 B, patron 3, en DOS tramos
//! ```

use super::*;
use bmo_block::{BlockError, DeviceId};

const DISCO: &[u8] = include_bytes!("../prueba/disco.ntfs");
/// Otro disco (clusteres de 512 B) con UN fichero disperso: 512 B escritos y
/// 512 de hueco, 800 veces -- 1606 tramos que no caben en un registro:
/// `$ATTRIBUTE_LIST` y su `$DATA` repartido en cinco registros.
const LISTA: &[u8] = include_bytes!("../prueba/lista.ntfs");

/// El disco del banco, de solo lectura, en bloques de 512.
struct Imagen(&'static [u8]);

impl BlockDevice for Imagen {
    fn identity(&self) -> DeviceId {
        DeviceId { blocks: self.0.len() as u64 / 512, ..DeviceId::EMPTY }
    }
    fn read(&self, lba: u64, count: u16, buf: &mut [u8]) -> Result<u16, BlockError> {
        let (desde, n) = (lba as usize * 512, count as usize * 512);
        if buf.len() < n {
            return Err(BlockError::ShortBuffer);
        }
        let d = self.0.get(desde..desde + n).ok_or(BlockError::OutOfRange)?;
        buf[..n].copy_from_slice(d);
        Ok(count)
    }
    fn write(&self, _: u64, _: u16, _: &[u8]) -> Result<u16, BlockError> {
        Err(BlockError::ReadOnly)
    }
    fn flush(&self) -> Result<(), BlockError> {
        Ok(())
    }
}

static IMAGEN: Imagen = Imagen(DISCO);

/// El contenido que escribio el guion: `(i*31 + k*7 + (i>>9)) & 0xFF`.
fn patron(n: usize, k: usize) -> Vec<u8> {
    (0..n).map(|i| ((i * 31 + k * 7 + (i >> 9)) & 0xFF) as u8).collect()
}

fn volumen() -> Box<Volumen<'static>> {
    Box::new(Volumen::montar(&IMAGEN, 0).expect("el disco de mkntfs se monta"))
}

fn entero(v: &mut Volumen, ruta: &str) -> Vec<u8> {
    let n = v.abrir(ruta).unwrap_or_else(|e| panic!("{ruta}: {}", e.nombre()));
    let mut b = vec![0u8; n.medida as usize];
    assert_eq!(v.leer(&n, 0, &mut b).unwrap(), b.len(), "{ruta}");
    b
}

#[test]
fn la_forma_del_volumen() {
    let v = volumen();
    let f = v.forma;
    assert_eq!((f.bytes_por_sector, f.bytes_por_cluster, f.registro, f.indice), (512, 4096, 1024, 4096));
    assert_eq!(f.sectores, 4 * 1024 * 1024 / 512 - 1, "mkntfs deja el ultimo sector para la copia del arranque");
}

#[test]
fn no_es_ntfs() {
    static CEROS: Imagen = Imagen(&[0u8; 4096]);
    assert!(matches!(Volumen::montar(&CEROS, 0), Err(NoNtfs::NoEsNtfs)));
}

#[test]
fn un_fichero_residente_y_uno_vacio() {
    let mut v = volumen();
    assert_eq!(entero(&mut v, "hola.txt"), b"hola desde NTFS\n");
    let n = v.abrir("/vacio.bin").unwrap();
    assert_eq!((n.medida, n.carpeta), (0, false));
    assert_eq!(v.leer(&n, 0, &mut [0u8; 8]).unwrap(), 0);
}

#[test]
fn ficheros_por_tramos_enteros_y_a_trozos() {
    let mut v = volumen();
    assert_eq!(entero(&mut v, "bin/x64/Cyberpunk2077.exe"), patron(70_000, 1));
    let ruta = "archive/pc/content/basegame_4_gamedata.archive";
    let esperado = patron(300_000, 2);
    assert_eq!(entero(&mut v, ruta), esperado);
    // A trozos que no caen en sectores ni en clusteres.
    let n = v.abrir(ruta).unwrap();
    let mut b = vec![0u8; 7777];
    let mut off = 0u64;
    while off < n.medida {
        let k = v.leer(&n, off, &mut b).unwrap();
        assert_eq!(&b[..k], &esperado[off as usize..off as usize + k], "desde {off}");
        off += k as u64;
    }
    assert_eq!(off, 300_000);
    assert_eq!(v.leer(&n, 300_000, &mut b).unwrap(), 0, "pasado el final, nada");
    assert_eq!(v.leer(&n, 299_990, &mut b).unwrap(), 10, "al final, lo que queda");
}

#[test]
fn un_fichero_partido_en_dos_tramos() {
    let mut v = volumen();
    assert_eq!(entero(&mut v, "frag/partido.bin"), patron(200_000, 3));
}

#[test]
fn las_rutas_sin_mayusculas_y_con_acentos() {
    let mut v = volumen();
    assert_eq!(entero(&mut v, "BIN\\X64\\cyberpunk2077.EXE"), patron(70_000, 1));
    assert_eq!(entero(&mut v, "Caf\u{e9} \u{f1}and\u{fa}.txt"), b"unicode\n");
    assert_eq!(entero(&mut v, "CAF\u{c9} \u{d1}AND\u{da}.TXT"), b"unicode\n", "Latin-1 sin mayusculas");
    assert!(matches!(v.abrir("no/existe.txt"), Err(NoNtfs::NoEsta)));
    assert!(matches!(v.abrir("hola.txt/dentro"), Err(NoNtfs::NoEsCarpeta)));
    let bin = v.abrir("bin").unwrap();
    assert!(bin.carpeta);
    assert!(matches!(v.leer(&bin, 0, &mut [0u8; 4]), Err(NoNtfs::NoEsFichero)));
}

#[test]
fn una_carpeta_grande_va_en_bloques_indx() {
    let mut v = volumen();
    let d = v.abrir("r6/scripts").unwrap();
    let mut nombres = Vec::new();
    v.recorrer(d.registro, &mut |e| {
        let mut b = [0u8; 256];
        let n = e.nombre_utf8(&mut b);
        nombres.push(String::from_utf8(b[..n].to_vec()).unwrap());
        false
    })
    .unwrap();
    nombres.sort();
    let esperados: Vec<String> = (0..80).map(|i| format!("script_numero_{i:03}_con_un_nombre_bastante_largo.reds")).collect();
    assert_eq!(nombres, esperados, "las 80, sin repetir y sin alias 8.3");
    for i in [0, 41, 79] {
        assert_eq!(entero(&mut v, &format!("r6/scripts/{}", esperados[i])), format!("n={i}\n").as_bytes());
    }
}

#[test]
fn lo_borrado_no_aparece() {
    let mut v = volumen();
    let d = v.abrir("frag").unwrap();
    let mut n = 0;
    v.recorrer(d.registro, &mut |_| {
        n += 1;
        false
    })
    .unwrap();
    assert_eq!(n, 61, "60 rellenos vivos y partido.bin");
    assert!(matches!(v.abrir("frag/r000.bin"), Err(NoNtfs::NoEsta)), "borrado");
    assert_eq!(entero(&mut v, "frag/r001.bin"), patron(16_384, 1));
    assert_eq!(entero(&mut v, "frag/r119.bin"), patron(16_384, 119));
}

#[test]
fn un_arreglo_que_no_cuadra_se_dice() {
    let mut r = [0u8; 1024];
    r[..4].copy_from_slice(b"FILE");
    r[4] = 0x30; // la tabla en +0x30
    r[6] = 3; // 1 + 2 tramos de 512
    r[0x30] = 0xAB;
    r[0x31] = 0xCD;
    r[510] = 0xAB;
    r[511] = 0xCD;
    r[1022] = 0xAB;
    r[1023] = 0xCD;
    r[0x32] = 1;
    r[0x34] = 2;
    let mut bien = r;
    arreglar(&mut bien, b"FILE").unwrap();
    assert_eq!((bien[510], bien[1022]), (1, 2), "los valores de verdad, en su sitio");
    let mut mal = r;
    mal[1023] = 0;
    assert!(matches!(arreglar(&mut mal, b"FILE"), Err(NoNtfs::Forma(_))), "escrito a medias");
    assert!(matches!(arreglar(&mut r.clone(), b"INDX"), Err(NoNtfs::Forma(_))), "otra firma");
}

#[test]
fn los_tramos_con_huecos_saltos_atras_y_mas_de_4_gib() {
    // 0x10 clusteres en 0x100; 0x10 de HUECO; 0x10 en 0x100 - 0x80 = 0x80
    // (salto atras); y 0x1_0000_0000 clusteres (16 TiB en 4 KiB: mas de 4 GiB
    // de sobra) en 0x80 + 0x1000.
    let t = [0x21, 0x10, 0x00, 0x01, 0x01, 0x10, 0x11, 0x10, 0x80, 0x25, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x10, 0x00];
    assert_eq!(mapear(&t, 0, 0).unwrap(), Some((Some(0x100), 0x10)));
    assert_eq!(mapear(&t, 0, 0x0F).unwrap(), Some((Some(0x10F), 1)));
    assert_eq!(mapear(&t, 0, 0x10).unwrap(), Some((None, 0x10)), "hueco");
    assert_eq!(mapear(&t, 0, 0x25).unwrap(), Some((Some(0x85), 0x0B)), "salto atras");
    let lejos = 0x30 + 0xFFFF_FFFF;
    assert_eq!(mapear(&t, 0, lejos).unwrap(), Some((Some(0x80 + 0x1000 + 0xFFFF_FFFF), 1)), "en u64");
    assert_eq!(mapear(&t, 0, 0x30 + 0x1_0000_0000).unwrap(), None, "fuera");
    assert!(mapear(&[0x19, 0], 0, 0).is_err(), "9 bytes de largo no caben en u64");
    assert!(mapear(&[0x11, 0x00, 0x05, 0], 0, 0).is_err(), "largo cero");
}

#[test]
fn un_fichero_en_cinco_registros_por_su_attribute_list_y_con_huecos() {
    static L: Imagen = Imagen(LISTA);
    let mut v = Box::new(Volumen::montar(&L, 0).unwrap());
    assert_eq!(v.forma.bytes_por_cluster, 512);
    let p = patron(800 * 1024, 5);
    let esperado: Vec<u8> = (0..800 * 1024).map(|i| if i % 1024 < 512 { p[i] } else { 0 }).collect();
    let n = v.abrir("disperso.bin").unwrap();
    assert_eq!(n.medida, 800 * 1024, "la medida, del $DATA de VCN 0");
    let mut b = vec![0u8; n.medida as usize];
    assert_eq!(v.leer(&n, 0, &mut b).unwrap(), b.len());
    assert!(b == esperado, "entero: los tramos de los cinco registros y los huecos a cero");
    // Un trozo que cae en la ULTIMA extension, sin pasar por las demas.
    let mut t = vec![0u8; 3000];
    let off = 780 * 1024 + 100;
    assert_eq!(v.leer(&n, off as u64, &mut t).unwrap(), 3000);
    assert_eq!(t, esperado[off..off + 3000]);
}

#[test]
fn el_espacio_libre_como_lo_cuenta_ntfsinfo() {
    // `ntfsinfo -m` de ntfs-3g sobre los dos discos: "Volume Size in
    // Clusters" y "Free Clusters". El numero de verdad lo pone otra herramienta.
    let mut v = volumen();
    assert_eq!((v.clusteres(), v.libres().unwrap()), (1023, 194));
    static L: Imagen = Imagen(LISTA);
    let mut l = Box::new(Volumen::montar(&L, 0).unwrap());
    assert_eq!((l.clusteres(), l.libres().unwrap()), (8191, 4374));
    // Y leer despues sigue leyendo: `libres` no deja el registro cambiado mal.
    assert_eq!(entero(&mut v, "hola.txt"), b"hola desde NTFS\n");
}

/// El mismo disco, contando cuantas lecturas le llegan.
struct Contada(std::sync::atomic::AtomicU64);

impl BlockDevice for Contada {
    fn identity(&self) -> DeviceId {
        IMAGEN.identity()
    }
    fn read(&self, lba: u64, count: u16, buf: &mut [u8]) -> Result<u16, BlockError> {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        IMAGEN.read(lba, count, buf)
    }
    fn write(&self, _: u64, _: u16, _: &[u8]) -> Result<u16, BlockError> {
        Err(BlockError::ReadOnly)
    }
    fn flush(&self) -> Result<(), BlockError> {
        Ok(())
    }
}

#[test]
fn abrir_otra_vez_la_misma_carpeta_no_toca_el_disco() {
    // El metal (30-09): abrir las 25 DLL de `bin/x64` recorria la misma
    // carpeta 25 veces, un comando por bloque INDX. Con la cache, la segunda
    // vez sale de la RAM.
    static DISCO_CONTADO: Contada = Contada(std::sync::atomic::AtomicU64::new(0));
    let mut v = Box::new(Volumen::montar(&DISCO_CONTADO, 0).unwrap());
    let cuenta = || DISCO_CONTADO.0.load(std::sync::atomic::Ordering::SeqCst);
    let recorrer = |v: &mut Volumen| {
        let d = v.abrir("r6/scripts").unwrap();
        let mut n = 0;
        v.recorrer(d.registro, &mut |_| {
            n += 1;
            false
        })
        .unwrap();
        n
    };
    let a = cuenta();
    let primera = recorrer(&mut v);
    let leidas = cuenta() - a;
    let b = cuenta();
    assert_eq!(recorrer(&mut v), primera, "las mismas entradas");
    assert_eq!(cuenta() - b, 0, "la segunda vez, ni una lectura (la primera: {leidas})");
    let (aciertos, fallos) = v.cache();
    assert!(aciertos > 0 && fallos > 0, "{aciertos} aciertos, {fallos} fallos");
    // Y lo que se lee sigue siendo lo del disco.
    assert_eq!(entero(&mut v, "BIN\\X64\\cyberpunk2077.EXE"), patron(70_000, 1));
}
