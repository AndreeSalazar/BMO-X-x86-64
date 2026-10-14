//! **B2: la autopsia dice la instruccion y su registro malo.**
//!
//! El kernel no corre en el anfitrion; lo que si corre es lo que la autopsia
//! hace con lo que captura: los bytes en el `rip` y los registros, leidos con
//! la lente (`bmo_lente_x86`, lo mismo que `core/autopsy.rs` llama). Aqui el
//! fallo lo da la TRAMPA del emulador -- un rango que, si el programa lo toca,
//! para la maquina como un `#PF` -- sobre codigo que emitio de verdad BMO C.

use super::*;

/// La direccion mala: alta, alineada y de nadie.
const MALA: u64 = 0xDEAD_0000;

/// ** Un programa que lee de un puntero malo A PROPOSITO: la instruccion que
/// fallo es una lectura de memoria, la lente la lee entera, y el registro de
/// su direccion -- con su valor al fallar -- da EXACTAMENTE la direccion que
/// se toco. Eso es lo que la linea `instr` de la autopsia escribe.
#[test]
fn the_autopsy_names_the_instruction_and_its_bad_register() {
    let src = "int leer(int *p) { return p[100]; }
               int main(void) { return leer((int *)0xDEAD0000); }";
    let bef = compile_source_to_bef(src).unwrap();
    let m = maquina_de_bef_con(&bef, |m| m.trampa = Some((MALA, MALA + 0x1000)));
    let f = m.fallo.expect("el programa tenia que tocar la trampa");
    assert_eq!(f.addr, MALA + 400, "p[100] son 400 bytes mas alla");
    let i = bmo_lente_x86::lee(&m.code[f.rip..]).expect("la lente tiene que leer la instruccion que fallo");
    let d = i.memoria().expect("la que fallo toca memoria");
    let base = d.base.expect("y su direccion sale de un registro");
    let calculada = d.indice.map_or(0, |x| f.regs[x as usize] * d.escala as u64)
        .wrapping_add(f.regs[base as usize])
        .wrapping_add(d.disp as u64);
    assert_eq!(calculada, f.addr, "`{}`: el registro y el desplazamiento dan la direccion tocada", i.texto(f.rip as u64).as_str());
    // Y el registro malo lleva el puntero que se paso.
    assert!(
        [base].into_iter().chain(d.indice).any(|r| f.regs[r as usize] == MALA || f.regs[r as usize] == MALA + 400),
        "{} = {:#x}",
        bmo_lente_x86::nombre_r64(base),
        f.regs[base as usize]
    );
}
