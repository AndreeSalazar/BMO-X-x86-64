//! Peticion EXEC_HS_BINARY 615 para el GSP de Ampere.
//! capa: puro -- valida TODA la peticion antes de producir ordenes acotadas.
//! [carril] AMARILLO
//! [estado]  VOLATIL  las ordenes actuan sobre GSP y su TCM; reset los borra.
//! Referencia: OpenRM 615 kernel_gsp_falcon_ga102.c y rmgspseq.h (MIT).
use crate::{falcon as f, secuenciador::Orden};

pub const PARAM_BYTES: usize = 80;
pub const MAX_ORDENES: usize = 5200;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum No { Medida, Reservado, Alineacion, Memoria, Motor, Espacio }
fn d32(b: &[u8], o: usize) -> u32 { u32::from_le_bytes(b[o..o+4].try_into().unwrap()) }
fn d64(b: &[u8], o: usize) -> u64 { u64::from_le_bytes(b[o..o+8].try_into().unwrap()) }

/// Solo VRAM local, nunca una direccion fisica del PC. TCM, codigo y datos
/// caben en las capacidades leidas del GSP; las firmas siguen ante el BROM.
pub fn ordenes(b: &[u8], vram: u64, imem: u32, dmem: u32, out: &mut [Orden]) -> Result<usize, No> {
    if b.len() != PARAM_BYTES { return Err(No::Medida); }
    if b[16..32].iter().chain(b[68..80].iter()).any(|v| *v != 0) { return Err(No::Reservado); }
    let (ip, dp) = (d64(b, 0), d64(b, 8));
    let (iv, ia, iz) = (d32(b, 32), d32(b, 36), d32(b, 40));
    let (dv, da, dz) = (d32(b, 44), d32(b, 48), d32(b, 52));
    let (sig, motor, id) = (d32(b, 56), d32(b, 60), d32(b, 64));
    let doff = if dv == u32::MAX { 0 } else { dv };
    if (ip | dp | iv as u64 | ia as u64 | iz as u64 | doff as u64 | da as u64 | dz as u64) & 255 != 0 || sig & 3 != 0 {
        return Err(No::Alineacion);
    }
    if motor != 0x400 || id > 255 { return Err(No::Motor); }
    if iz == 0 || dz == 0 || imem > 131072 || dmem > 131072
        || ia.checked_add(iz).filter(|e| *e <= imem).is_none()
        || da.checked_add(dz).filter(|e| *e <= dmem).is_none()
        || iv.checked_add(iz).filter(|e| *e <= 0x1000000).is_none()
        || doff.checked_add(dz).filter(|e| *e <= 0x1000000).is_none()
        || sig < da || sig.checked_add(16).filter(|e| *e <= da+dz).is_none() {
        return Err(No::Memoria);
    }
    for (base, offset, n) in [(ip, iv, iz), (dp, doff, dz)] {
        if base.checked_add(offset as u64).and_then(|v| v.checked_add(n as u64)).filter(|end| *end <= vram).is_none() {
            return Err(No::Memoria);
        }
    }
    let requerido = 18 + ((iz + dz) / 256) as usize * 5;
    if requerido > out.len() { return Err(No::Espacio); }
    let mut n = 0;
    let mut poner = |o| { out[n] = o; n += 1; };
    poner(Orden::Esperar { reg: f::GSP + f::RISCV_CPUCTL, mascara: 16, valor: 16, plazo_us: 2_000_000 });
    poner(Orden::Resetear);
    poner(Orden::Modificar { reg: f::GSP + f::TRANSCFG, mascara: 0x10007, valor: 4 }); // LOCAL_FB, PHYSICAL, BAR2_FN0
    for (base, offset, dest, nbytes, cmd) in [(ip, iv, ia, iz, 0x614), (dp, doff, da, dz, 0x600 | if dv != u32::MAX { 1 << 16 } else { 0 })] {
        poner(Orden::Esperar { reg: f::GSP + f::DMATRFCMD, mascara: 1, valor: 0, plazo_us: 20_000 });
        poner(Orden::Escribir { reg: f::GSP + f::DMATRFBASE, valor: (base >> 8) as u32 });
        poner(Orden::Escribir { reg: f::GSP + f::DMATRFBASE1, valor: ((base >> 40) & 0x1FF) as u32 });
        for k in (0..nbytes).step_by(256) {
            poner(Orden::Esperar { reg: f::GSP + f::DMATRFCMD, mascara: 1, valor: 0, plazo_us: 20_000 });
            poner(Orden::Escribir { reg: f::GSP + f::DMATRFMOFFS, valor: dest+k });
            poner(Orden::Escribir { reg: f::GSP + f::DMATRFFBOFFS, valor: offset+k });
            poner(Orden::Escribir { reg: f::GSP + f::DMATRFCMD, valor: cmd });
            poner(Orden::Esperar { reg: f::GSP + f::DMATRFCMD, mascara: 2, valor: 2, plazo_us: 20_000 });
        }
    }
    for (reg, valor) in [(f::BROM_PARAADDR, sig), (f::BROM_ENGIDMASK, motor),
                         (f::BROM_UCODE_ID, id), (f::MOD_SEL, 1), (f::MAILBOX0, 0xFE), (f::BOOTVEC, iv)] {
        poner(Orden::Escribir { reg: f::GSP + reg, valor });
    }
    poner(Orden::Arrancar); poner(Orden::EsperarParada); poner(Orden::Reanudar);
    Ok(n)
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use crate::{Registros, correr::{Contexto, Corredor, Tramo}};
    use std::vec::Vec;
    fn peticion() -> [u8; 80] {
        let mut b = [0; 80];
        b[..8].copy_from_slice(&0x2000u64.to_le_bytes());
        b[8..16].copy_from_slice(&0x4000u64.to_le_bytes());
        for (o, v) in [(40, 256u32), (44, u32::MAX), (52, 512), (60, 0x400), (64, 9)] {
            b[o..o+4].copy_from_slice(&v.to_le_bytes());
        }
        b
    }
    #[test]
    fn hs_solo_produce_ordenes_gsp_y_deja_la_firma_al_brom() {
        let mut out = [Orden::Retraso { us: 0 }; 64];
        let n = ordenes(&peticion(), 12 << 30, 65536, 65536, &mut out).unwrap();
        crate::correr::validar(&out[..n]).unwrap();
        assert!(out[..n].contains(&Orden::Escribir { reg: f::GSP+f::MOD_SEL, valor: 1 }));
        assert_eq!(out[n-1], Orden::Reanudar);
    }
    #[test]
    fn peticion_fuera_de_vram_o_tcm_no_produce_ni_una_orden() {
        let marca = Orden::Retraso { us: 123 };
        for (o, valor) in [(4, 0x100), (16, 1), (32, 1), (36, 65536), (40, 0),
                           (48, 65536), (56, 600), (60, 1), (68, 1)] {
            let mut b = peticion(); b[o..o+4].copy_from_slice(&(valor as u32).to_le_bytes());
            let mut out = [marca; 64];
            assert!(ordenes(&b, 12 << 30, 65536, 65536, &mut out).is_err(), "offset {o}");
            assert_eq!(out, [marca; 64]);
        }
    }
    #[test]
    fn hs_dma_firma_y_reanudacion_por_tramos_ante_aparato_simulado() {
        struct Placa { escrituras: Vec<(u32, u32)>, activo: bool }
        impl Registros for Placa {
            fn leer(&mut self, r: u32) -> u32 {
                if r == crate::aon::BSI_14 { return if self.activo { crate::aon::BSI_14_HANDOFF } else { 0 }; }
                if r == f::GSP+f::HWCFG2 { return 1 << 31; }
                if r == f::GSP+f::BCR_CTRL { return 1; }
                if r == f::GSP+f::RISCV_CPUCTL { return if self.activo { 0x80 } else { 0x10 }; }
                if r == f::GSP+f::CPUCTL { return 0x10; }
                if r == f::GSP+f::DMATRFCMD { return 2; }
                0
            }
            fn escribir(&mut self, r: u32, v: u32) {
                self.escrituras.push((r, v));
                if r == f::SEC2+f::CPUCTL && v == 2 { self.activo = true; }
            }
        }
        struct Tic(u64);
        impl f::Reloj for Tic { fn us(&mut self) -> u64 { self.0 += 10; self.0 } }
        let mut out = [Orden::Retraso { us: 0 }; 64];
        let n = ordenes(&peticion(), 12 << 30, 65536, 65536, &mut out).unwrap();
        let mut placa = Placa { escrituras: Vec::new(), activo: false };
        let mut c = Corredor::default(); let mut reloj = Tic(0);
        let ctx = Contexto { boot0: 0xB76000A1, libos: 0x3C000000, os: 123 };
        let mut finalizo = false;
        for _ in 0..100 {
            match c.correr(&out[..n], &mut placa, &mut reloj, &ctx, 100) {
                Tramo::Hecho => { finalizo = true; break; },
                Tramo::Sigue => {}, otro => panic!("{otro:?}"),
            }
        }
        assert!(finalizo && placa.activo);
        assert!(placa.escrituras.contains(&(f::GSP+f::BROM_ENGIDMASK, 0x400)));
        assert!(placa.escrituras.contains(&(f::GSP+f::DMATRFCMD, 0x614)));
        assert!(placa.escrituras.contains(&(f::GSP+f::OS, 123)));
        assert!(placa.escrituras.iter().all(|(r, _)| (*r >= f::GSP && *r <= f::GSP+0x1FFF) || *r == f::SEC2+f::CPUCTL));
    }
}
