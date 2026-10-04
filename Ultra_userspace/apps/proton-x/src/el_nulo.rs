//! **El salto a 0 de la corrida ANTERIOR, explicado al arrancar** (03-10,
//! N4.4).
//!
//! [carril]  VERDE     lee un fichero de texto y la imagen ya colocada
//! [cuesta]  NADA      si algo no cuadra se dice y se sigue: es un informe,
//!                     no un requisito para arrancar
//! [riesgo]  AJENO     el informe es de OTRA corrida: se comprueba que los
//!                     bytes del `call` siguen donde estaban antes de creerlo
//! [consumo] NADA      una vez al arrancar; el escaneo del codigo, solo si
//!                     el ultimo fallo fue un salto a 0 por `call [rip+..]`
//!
//! La autopsia del kernel conoce el `.bex` y no el `.exe`: dice "salto a 0
//! desde 0x1001d4c6cf" y los bytes de antes (`ff 15 ..`). PROTON-X si conoce
//! el `.exe`, y al arrancar de nuevo tiene ese mismo informe en el disco
//! (el director lo deja en `datos/fallos.txt`). Aqui se junta todo:
//!
//! ```text
//!    la casilla      que modulo, que RVA, que seccion
//!    que es         una importacion (dll!funcion) o una variable del .exe
//!    quien la toca   cada instruccion que la apunta (bmo_proton_x::nulo),
//!                    y de las que ESCRIBEN, el nombre que se paso en rdx
//!                    y la importacion que se llamo justo antes
//! ```
//!
//! Y deja la casilla VIGILADA: cada foto del pulso dice cuanto vale.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use bmo_proton_x::nulo::{self, Toque};
use bmo_proton_x::Permiso;
use bmo_userland as bmo;

use crate::cargador::Modulo;
use crate::di;

/// Donde deja el director las autopsias (`services/director/src/main.rs`).
const FALLOS: &[u8] = b"datos/fallos.txt";

/// Un informe de autopsias son cuatro fallos de una docena de renglones.
const TOPE: u64 = 64 << 10;

/// El informe del DISCO (la corrida anterior, aunque se reiniciara), o el
/// codigo del NO.
fn del_disco() -> Result<String, u32> {
    let a = bmo::Archivo::leer_de(FALLOS)?;
    let n = a.size().min(TOPE) as usize;
    let mut v = alloc::vec![0u8; n];
    let k = a.read(&mut v);
    v.truncate(k);
    Ok(String::from_utf8_lossy(&v).into_owned())
}

/// Las autopsias que el KERNEL guarda de este arranque (las 4 ultimas), en
/// el mismo formato que el fichero y en su orden: la mas reciente, al final.
fn del_kernel() -> String {
    let mut t = String::new();
    let mut buf = [0u8; 96];
    for i in (0..bmo::autopsia_disponibles()).rev() {
        for f in 0..bmo::autopsia_renglones(i) {
            let n = bmo::autopsia_linea(i, f, &mut buf);
            t.push_str(&String::from_utf8_lossy(&buf[..n]));
            t.push('\n');
        }
    }
    t
}

fn modulo_de(modulos: &[Modulo], dir: u64) -> Option<(&Modulo, u32)> {
    modulos.iter().find_map(|m| {
        let len = m.imagen().len() as u64;
        (dir >= m.base && dir - m.base < len).then(|| (m, (dir - m.base) as u32))
    })
}

fn corto(m: &Modulo) -> &str {
    m.nombre.rsplit('/').next().unwrap_or(&m.nombre)
}

/// **Mirar el informe y decirlo.** Antes de la entrada del `.exe`, con todo
/// colocado y resuelto.
pub(crate) fn mirar(modulos: &[Modulo]) {
    // ** Se dice SIEMPRE que se miro, y que salio (metal 04-10: la primera
    // version callaba si no podia leer, y la corrida no dijo nada).
    // Primero el kernel (este arranque); si no tiene, el disco.
    let (s, de) = match nulo::del_informe(&del_kernel()) {
        Some(s) => (s, "las autopsias del kernel de este arranque"),
        None => match del_disco() {
            Err(c) => {
                di(&format!(
                    "PROTON-X: el lector del nulo (N4.4): datos/fallos.txt no se abre (codigo {c}) y el kernel no tiene un salto a 0 de este arranque\n"
                ));
                return;
            }
            Ok(t) => match nulo::del_informe(&t) {
                Some(s) => (s, "datos/fallos.txt"),
                None => {
                    di(&format!(
                        "PROTON-X: el lector del nulo (N4.4): datos/fallos.txt ({} B) no acaba en un salto a 0 por `call [rip+..]`\n",
                        t.len()
                    ));
                    return;
                }
            },
        },
    };
    di(&format!(
        "PROTON-X: el lector del nulo (N4.4): en {de} hay un SALTO A 0 por `call [rip+..]` (retorno {:#x}, casilla {:#x}); se mira aqui\n",
        s.retorno, s.casilla
    ));
    let (Some((mr, rr)), Some((mc, rc))) = (modulo_de(modulos, s.retorno), modulo_de(modulos, s.casilla)) else {
        di(&format!(
            "PROTON-X:   el retorno {:#x} o la casilla {:#x} caen fuera de esta imagen: era otro programa\n",
            s.retorno, s.casilla
        ));
        return;
    };
    // El informe es de otra corrida: que los bytes del `call` sigan ahi.
    let img = mr.imagen();
    let llamada = (rr as usize).checked_sub(6).and_then(|i| img.get(i..i + 6));
    let disp = (s.casilla as i64 - s.retorno as i64) as i32;
    let mut esperado = [0xFFu8, 0x15, 0, 0, 0, 0];
    esperado[2..].copy_from_slice(&disp.to_le_bytes());
    if llamada != Some(&esperado[..]) {
        di(&format!(
            "PROTON-X:   {}+{rr:#x} ya no tiene ese `call` delante: el .exe cambio o quedo en otro sitio\n",
            corto(mr)
        ));
        return;
    }
    let img = mc.imagen();
    let seccion = mc
        .pe
        .secciones
        .iter()
        .find(|x| (x.rva..x.rva + x.tam_en_imagen()).contains(&rc))
        .map(|x| x.nombre.as_str())
        .unwrap_or("fuera de toda seccion");
    let ahora = img
        .get(rc as usize..rc as usize + 8)
        .map(|b| u64::from_le_bytes(b.try_into().unwrap_or([0; 8])))
        .unwrap_or(0);
    di(&format!(
        "PROTON-X:   la llamada: {}+{:#x}; la casilla: {}+{rc:#x} (seccion {seccion}); ahora, antes de la entrada, vale {ahora:#x}\n",
        corto(mr),
        rr - 6,
        corto(mc)
    ));
    let imps = bmo_proton_x::importaciones(&mc.pe, img).unwrap_or_default();
    let ret = bmo_proton_x::retrasadas_de_seccion(&mc.pe, img, 0).unwrap_or_default();
    let nombre_de = |rva: u32| -> Option<String> {
        imps.iter()
            .map(|i| (i, ""))
            .chain(ret.iter().map(|i| (i, " (retrasada)")))
            .find(|(i, _)| i.ranura == rva)
            .map(|(i, r)| format!("{}!{}{r}", i.dll, i.funcion))
    };
    match nombre_de(rc) {
        Some(n) => di(&format!("PROTON-X:   es una IMPORTACION: {n}\n")),
        None => di("PROTON-X:   NO es una importacion: es una variable del modulo (un puntero a funcion que alguien llena)\n"),
    }
    // Quien la toca, seccion de codigo a seccion de codigo.
    let mut refs = Vec::new();
    for x in mc.pe.secciones.iter().filter(|x| x.permiso() == Permiso::Codigo) {
        let (a, b) = (x.rva as usize, (x.rva + x.tam_en_imagen()) as usize);
        if let Some(codigo) = img.get(a..b.min(img.len())) {
            refs.extend(nulo::referencias(codigo, x.rva, rc));
        }
    }
    if refs.is_empty() {
        di("PROTON-X:   ninguna instruccion del codigo la apunta con [rip+..]: se llena desde fuera (otra DLL, o por un puntero)\n");
    }
    for r in refs.iter().take(nulo::TOPE) {
        let mut l = format!("PROTON-X:   {}+{:#x} {}", corto(mc), r.rva, r.toque.nombre());
        if r.toque == Toque::Escribe {
            if let Some(c) = r.cadena.and_then(|c| nulo::cadena_en(img, c)) {
                l.push_str(&format!(" -- con \"{c}\" en rdx"));
            }
            if let Some(n) = r.tras_llamar.and_then(nombre_de) {
                l.push_str(&format!(" -- despues de llamar a {n}"));
            }
        }
        l.push('\n');
        di(&l);
    }
    bmo_proton_x_casa::pulso::vigilar(s.casilla);
}
