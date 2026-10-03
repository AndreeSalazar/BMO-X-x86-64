//! El salto a 0 (N4.4): el informe del metal y un codigo hecho a mano.

use alloc::vec::Vec;

use crate::nulo::{cadena_en, del_informe, referencias, SaltoNulo, Toque};

/// El `fallos.txt` del metal del 03-10, tal cual.
const INFORME: &str = "== FALLO EN RING 3 #1  t=53246ms ==
programa  proton-x.bex   pid 2 tid 7
causa     #PF fallo de pagina  (vector 14)
veredicto *** PUNTERO NULO en 0+0x0
codigo    0x15  violacion de permisos, leyendo, desde Ring 3, buscando I
rip       0x0  (FUERA de la imagen)
direccion 0x0   lo que se intento tocar
rsp       0x20041ff0d8
ultimo    ida: todavia no\")
recursos  todo devuelto
pila      SIN retornos en 32 palabras: 0x1001d4c6cf 0x340d7f9488 0x401b0
antes     20 02 00 01 00 48 89 4c 24 28 ff 15 29 82 83 01 | ret (el call
regs      rax 0x20041ff140  rbx 0x202790fea0  rcx 0x0
";

#[test]
fn la_casilla_del_metal() {
    let s = del_informe(INFORME).unwrap();
    assert_eq!(s, SaltoNulo { retorno: 0x10_01d4_c6cf, casilla: 0x10_0358_48f8 });
}

#[test]
fn otro_fallo_no_da_casilla() {
    // Un #PF con `rip` dentro del programa no es un salto a 0.
    let otro = INFORME.replace("rip       0x0  (FUERA", "rip       0x1000001234  (DENTRO");
    assert_eq!(del_informe(&otro), None);
    // Un salto a 0 por `call rax` (ff d0) no tiene casilla.
    let reg = INFORME.replace("ff 15 29 82 83 01 |", "00 00 00 00 ff d0 |");
    assert_eq!(del_informe(&reg), None);
    assert_eq!(del_informe(""), None);
}

#[test]
fn de_varios_vale_el_ultimo() {
    let dos = alloc::format!("{INFORME}\n{}", INFORME.replace("ff 15 29 82 83 01", "ff 15 00 01 00 00"));
    assert_eq!(del_informe(&dos).unwrap().casilla, 0x10_01d4_c6cf + 0x100);
}

fn pon_disp(c: &mut Vec<u8>, desde: u32, fin_extra: u32, objetivo: u32) {
    // El desplazamiento cuenta desde el FINAL de la instruccion.
    let fin = desde + c.len() as u32 + 4 + fin_extra;
    c.extend_from_slice(&(objetivo.wrapping_sub(fin)).to_le_bytes());
}

#[test]
fn quien_toca_la_casilla() {
    const DESDE: u32 = 0x1000;
    const CASILLA: u32 = 0x9000;
    const GPA: u32 = 0x8000;
    const NOMBRE: u32 = 0x7000;
    let mut c = Vec::new();
    c.extend_from_slice(&[0x90; 8]);
    // lea rdx, [rip+NOMBRE]
    c.extend_from_slice(&[0x48, 0x8D, 0x15]);
    pon_disp(&mut c, DESDE, 0, NOMBRE);
    // call [rip+GPA]
    c.extend_from_slice(&[0xFF, 0x15]);
    pon_disp(&mut c, DESDE, 0, GPA);
    // mov [rip+CASILLA], rax
    let escribe = DESDE + c.len() as u32 + 1;
    c.extend_from_slice(&[0x48, 0x89, 0x05]);
    pon_disp(&mut c, DESDE, 0, CASILLA);
    // mov qword [rip+CASILLA], 0  (lleva un inmediato de 4 detras)
    let pone_cero = DESDE + c.len() as u32 + 1;
    c.extend_from_slice(&[0x48, 0xC7, 0x05]);
    pon_disp(&mut c, DESDE, 4, CASILLA);
    c.extend_from_slice(&[0, 0, 0, 0]);
    // mov rax, [rip+OTRA]: no es ella
    c.extend_from_slice(&[0x48, 0x8B, 0x05]);
    pon_disp(&mut c, DESDE, 0, CASILLA + 8);
    // call [rip+CASILLA]
    let llama = DESDE + c.len() as u32;
    c.extend_from_slice(&[0xFF, 0x15]);
    pon_disp(&mut c, DESDE, 0, CASILLA);
    c.extend_from_slice(&[0xC3]);

    let v = referencias(&c, DESDE, CASILLA);
    assert_eq!(v.len(), 3, "{v:?}");
    assert_eq!((v[0].rva, v[0].toque), (escribe, Toque::Escribe));
    assert_eq!(v[0].cadena, Some(NOMBRE));
    assert_eq!(v[0].tras_llamar, Some(GPA));
    assert_eq!((v[1].rva, v[1].toque), (pone_cero, Toque::Escribe));
    assert_eq!((v[2].rva, v[2].toque), (llama, Toque::Llama));
    assert_eq!((v[2].cadena, v[2].tras_llamar), (None, None));
}

#[test]
fn la_cadena_si_parece_un_nombre() {
    let mut img = alloc::vec![0u8; 64];
    img[8..8 + 30].copy_from_slice(b"WTSRegisterSessionNotification");
    assert_eq!(cadena_en(&img, 8), Some("WTSRegisterSessionNotification"));
    img[40] = 0xC3;
    img[41] = 0x90;
    assert_eq!(cadena_en(&img, 40), None);
    assert_eq!(cadena_en(&img, 1000), None);
}
