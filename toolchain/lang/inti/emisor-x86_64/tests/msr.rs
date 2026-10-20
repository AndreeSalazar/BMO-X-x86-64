//! PP3: INTI cruza la puerta; el emulador solo devuelve valores MODELADOS.
use bmo_lower::emu::{run, run_acotado, Machine};
use bmo_lower::x86::{RAX, RDI, RDX, RSI};
use bmo_abi::syscalls::surface::*;

fn corre(f: &str) -> Machine {
    let arbol = bmo_inti_front::armar(f);
    assert!(!arbol.hay_errores(), "{}", arbol.pintar("msr.inti"));
    let raices = bmo_mods::Roots::find();
    let modulos = bmo_inti_front::tablas::Modulos::cargar(&raices);
    let plano = bmo_inti_front::disposicion::comprobar(&arbol.valor, bmo_inti_front::disposicion::Medidas::cargar(&raices));
    let metal = bmo_inti_front::ir::metal_que_declara(&arbol.valor, &raices, &modulos);
    let nec = bmo_inti_front::necesidades::Necesidades::por_defecto();
    let ir = bmo_inti_front::ir::bajar_con(&arbol.valor, &modulos, &plano.valor, &metal, &nec);
    assert!(!ir.hay_errores(), "{}", ir.pintar("msr.inti"));
    run(Machine::new(bmo_inti_x86_64::emitir(&ir.valor).codigo), 5_000_000)
}

#[test]
fn inti_reads_the_modelled_microcode_and_rejects_efer() {
    let m = corre("perfil llano\nusa bmo\nfuncion principal devuelve natural64\n    v es natural64 = invoca_valor(mi_tarea, op_lee_msr, 139, 0, 0)\n    c es natural64 = invoca(mi_tarea, op_lee_msr, 3221225600, 0, 0)\n    si c = error_negado\n        devuelve v\n    devuelve 0\n");
    assert!(m.exited);
    assert_eq!(m.syscalls.last().unwrap().arg0, 0x454D_5500_0000_008B);
}

#[test]
fn raw_status_preserves_the_denial_reason_and_does_not_truncate_the_msr() {
    for msr in [0xC000_0080, 0xC001_029A, 0xC001_0200, 0x1_0000_008B] {
        let mut m = Machine::new(vec![0x0F, 0x05]);
        m.regs[RAX as usize] = NR_INVOKE as u64;
        m.regs[RDI as usize] = CURRENT_TASK;
        m.regs[RSI as usize] = TASK_OP_LEE_MSR;
        m.regs[RDX as usize] = msr;
        let (m, _) = run_acotado(m, 1);
        assert_eq!(m.regs[RAX as usize], u64::from(ERROR_NEGADO) | (u64::from(LEE_MSR_FUERA) << 32));
    }
}

#[test]
fn the_hardware_probe_builds_and_visits_every_white_row() {
    let m = corre(include_str!("../../sondas/msr.inti"));
    assert!(m.exited);
    let asked: Vec<_> = m.syscalls.iter().filter(|s| s.operation == TASK_OP_LEE_MSR).map(|s| s.arg0).collect();
    let filas = [0x8B, 0xE7, 0xE8, 0xC001_0061, 0xC001_0063,
        0xC001_0064, 0xC001_0065, 0xC001_0066, 0xC001_0067,
        0xC001_0068, 0xC001_0069, 0xC001_006A, 0xC001_006B];
    // Cada fila comprueba el codigo antes de pedir el valor.
    assert_eq!(asked, filas.into_iter().flat_map(|m| [m, m]).collect::<Vec<_>>());
    assert!(m.console.contains("454d5500"), "el valor debe decir EMU");
}
