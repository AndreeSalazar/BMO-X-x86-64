//! # inti-hex -- el codigo de INTI, en el `.hex` de TALK-CPU
//!
//! 18-10, el propietario: *"este es mi repositorio que estoy hablando con la
//! CPU directo con hex"* (`AndreeSalazar/TALK-CPU-x86-64`). Alli cada byte de
//! un programa se escribe a mano, con una sintaxis de cuatro reglas, y un
//! puente `.efi` (`Puente_Metal`) carga un paquete FAGRAW64, cierra
//! BootServices y lo corre en Ring 0. Esto une las dos casas:
//!
//! ```text
//!    volcar   lo que EMITE INTI (sus funciones, sus katanas, sus huecos)
//!             escrito como .hex de TALK-CPU: se lee, se compara, se aprende
//!    leer     un .hex de vuelta a bytes, con las mismas reglas que su
//!             build.ps1 (y el NO en la linea exacta)
//!    fagraw   un cuerpo hecho paquete FAGRAW64 v1: la longitud, la entrada
//!             y la suma CALCULADAS -- lo que hoy se cuenta a mano en cada
//!             `*-header.hex`
//! ```
//!
//! La sintaxis de TALK-CPU (`src/hola.hex`, su cabecera):
//!
//! ```text
//!    4D 5A     un byte en hexadecimal (siempre 2 cifras)
//!    ; texto   comentario hasta el final de la linea
//!    @200      rellena con 00 hasta el offset 0x200
//!    =058      comprueba que se esta EXACTAMENTE en el offset 0x58
//! ```
//!
//! [!] Lo que NO hace, dicho: un programa de INTI de hoy es de BMO-X -- sus
//! trampas y su `invoca` acaban en un `syscall`, y sus tablas en huecos que
//! rellena el cargador del kernel --, asi que su volcado se LEE, no se corre en
//! el puente. `volcar` lista cada hueco y cada `syscall` que encuentra en el
//! cuerpo, para que nadie lo empaquete a ciegas.

use std::fmt::Write;

/// La firma de un paquete FAGRAW64 v1.
pub const FIRMA: &[u8; 8] = b"FAGRAW64";
/// Lo que mide su cabecera.
pub const CABECERA: usize = 24;
/// El cuerpo mas largo que acepta el puente (su pagina de 4096, menos la cabecera).
pub const CUERPO_MAX: usize = 4072;

/// **Un `.hex` de TALK-CPU, leido**: sus bytes, o el NO con su linea (desde 1).
pub fn leer(texto: &str) -> Result<Vec<u8>, String> {
    let mut b = Vec::new();
    for (n, linea) in texto.lines().enumerate() {
        let no = |por: String| format!("linea {}: {}", n + 1, por);
        let util = linea.split(';').next().unwrap_or("");
        for pieza in util.split_whitespace() {
            if let Some(h) = pieza.strip_prefix('@') {
                let o = usize::from_str_radix(h, 16).map_err(|_| no(format!("`@{h}` no es un offset hexadecimal")))?;
                if o < b.len() {
                    return Err(no(format!("`@{h}` va hacia atras: ya hay {:#x} bytes", b.len())));
                }
                b.resize(o, 0);
            } else if let Some(h) = pieza.strip_prefix('=') {
                let o = usize::from_str_radix(h, 16).map_err(|_| no(format!("`={h}` no es un offset hexadecimal")))?;
                if o != b.len() {
                    return Err(no(format!("`={h}` dice el offset {:#x}, y se esta en el {:#x}", o, b.len())));
                }
            } else if pieza.len() == 2 {
                b.push(u8::from_str_radix(pieza, 16).map_err(|_| no(format!("`{pieza}` no es un byte hexadecimal")))?);
            } else {
                return Err(no(format!("`{pieza}`: un byte son dos cifras hexadecimales")));
            }
        }
    }
    Ok(b)
}

/// Los bytes de `b`, de 16 en 16, cada fila con su `=offset` delante cuando
/// empieza una de las `marcas` (offset, comentario).
fn filas(s: &mut String, b: &[u8], base: usize, marcas: &[(usize, String)]) {
    let mut i = 0;
    while i < b.len() {
        let aqui = base + i;
        for (_, c) in marcas.iter().filter(|(o, _)| *o == aqui) {
            let _ = writeln!(s, "\n; ---- {c}");
        }
        if marcas.iter().any(|(o, _)| *o == aqui) || i == 0 {
            let _ = writeln!(s, "={aqui:03X}");
        }
        // hasta la siguiente marca, o 16 bytes
        let fin = marcas.iter().map(|(o, _)| *o).filter(|o| *o > aqui && *o < base + b.len()).min().map_or(b.len(), |o| o - base).min(i + 16);
        let fila: Vec<String> = b[i..fin].iter().map(|x| format!("{x:02X}")).collect();
        let _ = writeln!(s, "{}", fila.join(" "));
        i = fin;
    }
}

/// **El paquete FAGRAW64 v1** de `cuerpo` con su entrada en `entrada`: la
/// cabecera (firma, longitud, entrada, suma modulo 2^32, flags a cero) y el
/// cuerpo. El NO si el puente lo rechazaria.
pub fn fagraw(cuerpo: &[u8], entrada: u32) -> Result<Vec<u8>, String> {
    if cuerpo.is_empty() || cuerpo.len() > CUERPO_MAX {
        return Err(format!("el cuerpo mide {} bytes, y el puente acepta de 1 a {}", cuerpo.len(), CUERPO_MAX));
    }
    if entrada as usize >= cuerpo.len() {
        return Err(format!("la entrada {:#x} no esta dentro del cuerpo ({} bytes)", entrada, cuerpo.len()));
    }
    let suma = cuerpo.iter().fold(0u32, |s, x| s.wrapping_add(*x as u32));
    let mut p = Vec::with_capacity(CABECERA + cuerpo.len());
    p.extend_from_slice(FIRMA);
    p.extend_from_slice(&(cuerpo.len() as u32).to_le_bytes());
    p.extend_from_slice(&entrada.to_le_bytes());
    p.extend_from_slice(&suma.to_le_bytes());
    p.extend_from_slice(&0u32.to_le_bytes());
    p.extend_from_slice(cuerpo);
    Ok(p)
}

/// El paquete escrito como `.hex`, con cada campo de la cabecera comentado.
pub fn fagraw_hex(paquete: &[u8], de_donde: &str) -> String {
    let campo = |o: usize| u32::from_le_bytes(paquete[o..o + 4].try_into().expect("cabecera de 24 bytes"));
    let bytes = |r: std::ops::Range<usize>| paquete[r].iter().map(|x| format!("{x:02X}")).collect::<Vec<_>>().join(" ");
    let mut s = String::new();
    let _ = writeln!(s, "; FAGRAW64 v1 de {de_donde}, escrito por inti-hex (BMO-X): la longitud,");
    let _ = writeln!(s, "; la entrada y la suma estan CALCULADAS. Sin PE/EFI.");
    let _ = writeln!(s, "=000\n{}                ; \"FAGRAW64\"", bytes(0..8));
    let _ = writeln!(s, "{}                         ; longitud del cuerpo: {}", bytes(8..12), campo(8));
    let _ = writeln!(s, "{}                         ; entrada: {:#x}", bytes(12..16), campo(12));
    let _ = writeln!(s, "{}                         ; suma de sus bytes: {:#x}", bytes(16..20), campo(16));
    let _ = writeln!(s, "{}                         ; flags: cero", bytes(20..24));
    filas(&mut s, &paquete[CABECERA..], CABECERA, &[(CABECERA, "el cuerpo".to_string())]);
    let _ = writeln!(s, "={:03X}", paquete.len());
    s
}

/// Donde hay un `syscall` (0F 05) en el cuerpo: lo que en el puente no tiene
/// a quien llamar. Es una busqueda de bytes, no un desensamblado: puede dar
/// un falso positivo dentro de un inmediato, nunca se salta uno de verdad.
pub fn syscalls(b: &[u8]) -> Vec<usize> {
    b.windows(2).enumerate().filter(|(_, w)| w == &[0x0F, 0x05]).map(|(i, _)| i).collect()
}

/// **El volcado de un modulo de INTI** como `.hex` de TALK-CPU.
pub fn volcar(c: &bmo_inti_x86_64::cadena::Compilado, de_donde: &str) -> String {
    let e = &c.emitido;
    let mut s = String::new();
    let _ = writeln!(s, "; =====================================================================");
    let _ = writeln!(s, ";  {de_donde}, emitido por INTI (BMO-X) y escrito por inti-hex.");
    let _ = writeln!(s, ";  {} bytes de codigo, {} funciones, {} comprobaciones anti-UB en bytes.", e.codigo.len(), e.inicios.len(), e.comprobaciones);
    let _ = writeln!(s, ";");
    let _ = writeln!(s, ";  Las funciones (offset dentro del codigo):");
    let mut inicios = e.inicios.clone();
    inicios.sort_by_key(|(_, o)| *o);
    for (n, o) in &inicios {
        let _ = writeln!(s, ";    {o:#06x}  {n}");
    }
    if !e.katanas.is_empty() {
        let _ = writeln!(s, ";\n;  Las katanas (donde corta cada regla):");
        for k in &e.katanas {
            let _ = writeln!(s, ";    {k:?}");
        }
    }
    let huecos = e.reubicaciones.len() + e.reubicaciones_del_monton.len();
    let llamadas = syscalls(&e.codigo);
    let _ = writeln!(s, ";");
    let _ = writeln!(s, ";  [!] Esto se LEE: es un programa de BMO-X. {huecos} hueco(s) los rellena el");
    let _ = writeln!(s, ";  cargador del kernel, y {} `syscall` (0F 05) llaman al kernel; en el", llamadas.len());
    let _ = writeln!(s, ";  Puente al Metal no hay quien los atienda.");
    for (o, t) in &e.reubicaciones {
        let _ = writeln!(s, ";    hueco en {o:#06x}: la direccion de la tabla {t}");
    }
    for o in &e.reubicaciones_del_monton {
        let _ = writeln!(s, ";    hueco en {o:#06x}: el slot del monton de la tarea");
    }
    for o in &llamadas {
        let _ = writeln!(s, ";    syscall en {o:#06x}");
    }
    let _ = writeln!(s, "; =====================================================================");
    let marcas: Vec<(usize, String)> = inicios.iter().map(|(n, o)| (*o, format!("fn {n}"))).collect();
    filas(&mut s, &e.codigo, 0, &marcas);
    let _ = writeln!(s, "={:03X}", e.codigo.len());
    s
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// El cuerpo de `Orquestador_Metal/64-bit/programs/hola.hex` de TALK-CPU:
    /// LEA RSI,[RIP+5]; CALL [R15+16]; RET; y su texto.
    const HOLA: &str = "48 8D 35 05 00 00 00\n41 FF 57 10\nC3\n48 6F 6C 61 20 6D 75 6E 64 6F 20 64 65 73 64 65 20 6D 69 20 2E 68 65 78 21 0D 0A 00\n";

    /// La cabecera que TALK-CPU escribio A MANO para ese cuerpo
    /// (`hola-header.hex`): 0x28 bytes, entrada 0, suma 0x0C26.
    const CABECERA_DE_TALK: [u8; 24] = [0x46, 0x41, 0x47, 0x52, 0x41, 0x57, 0x36, 0x34, 0x28, 0, 0, 0, 0, 0, 0, 0, 0x26, 0x0C, 0, 0, 0, 0, 0, 0];

    #[test]
    fn the_package_of_hola_is_the_one_talk_cpu_wrote_by_hand() {
        let cuerpo = leer(HOLA).unwrap();
        assert_eq!(cuerpo.len(), 40);
        let p = fagraw(&cuerpo, 0).unwrap();
        assert_eq!(&p[..24], &CABECERA_DE_TALK);
        assert_eq!(p.len(), 64, "el paquete exportado mide 64 bytes, como dice su README");
        // y su .hex se lee de vuelta, byte a byte
        assert_eq!(leer(&fagraw_hex(&p, "hola")).unwrap(), p);
    }

    #[test]
    fn the_rules_of_the_hex_say_no_on_their_line() {
        assert_eq!(leer("4D 5A\n@03C\n40 00 00 00\n=040\n").unwrap().len(), 0x40);
        assert!(leer("4D 5A\n=003\n").unwrap_err().starts_with("linea 2"));
        assert!(leer("4D 5A ; ok\n4\n").unwrap_err().starts_with("linea 2"));
        assert!(leer("4D 5A 5A\n@001\n").unwrap_err().contains("hacia atras"));
        assert!(leer("ZZ\n").is_err());
    }

    #[test]
    fn the_bridge_rejects_what_fagraw_does_not_write() {
        assert!(fagraw(&[], 0).is_err());
        assert!(fagraw(&[0xC3], 1).is_err());
        assert!(fagraw(&vec![0x90; CUERPO_MAX + 1], 0).is_err());
        assert!(fagraw(&vec![0x90; CUERPO_MAX], 0).is_ok());
    }

    /// Un modulo de INTI de verdad: su volcado se lee de vuelta con los MISMOS
    /// bytes que emitio, con un `=offset` en cada funcion.
    #[test]
    fn an_inti_module_dumps_and_reads_back_byte_for_byte() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../lang/inti/sondas/cpu.inti");
        let texto = std::fs::read_to_string(&dir).unwrap();
        let c = bmo_inti_x86_64::cadena::compilar(&texto, "cpu.inti", &bmo_mods::Roots::find()).unwrap_or_else(|f| panic!("{f}"));
        let hex = volcar(&c, "sondas/cpu.inti");
        assert_eq!(leer(&hex).unwrap(), c.emitido.codigo);
        assert!(c.emitido.inicios.iter().all(|(n, _)| hex.contains(&format!("; ---- fn {n}"))));
    }
}
