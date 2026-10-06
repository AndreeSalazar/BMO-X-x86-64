//! **Copiar texturas de verdad** (02-10): cualquier subrecurso (mip, capa,
//! cara de un cubo, rebanada 3D), con su caja y su desplazamiento, entre
//! una textura y un bufer (`CopyTextureRegion`, las dos direcciones), entre
//! dos texturas, y con la memoria del `.exe` (`WriteToSubresource`,
//! `ReadFromSubresource`). Hasta hoy: la textura entera, subrecurso 0, y
//! solo de 4 bytes por pixel.
//!
//! ```text
//!    hacia la textura   cada texel NATIVO se pasa a lo interno
//!                       (`subrecursos::a_interno`); un BC, sus bloques tal cual
//!    desde la textura   exacto si lo interno es lo nativo (RGBA8, BGRA8, R32,
//!                       D32, BC) o son los cuatro floats de un formato de
//!                       float (N5.16: se vuelven a su formato); de lo demas
//!                       se dice y no se copia
//!    entre texturas     lo interno tal cual (mismo almacen)
//! ```
//!
//! Las coordenadas de un BC van en bloques: una caja de texeles se lleva a
//! los bloques que toca, como en D3D12 (que las pide alineadas a 4).

use crate::com::de;
use crate::d3d12::{Recurso, Tex};
use crate::subrecursos::{a_interno, interno_es_nativo, Almacen};

/// **Memoria lineal**: filas de `fila` bytes, rebanadas de `capa`, y lo
/// que se puede tocar desde `p`. En `formato` NATIVO, o ya `interno` (otra
/// textura de la casa: se copia tal cual).
pub(crate) struct Lineal {
    pub p: *mut u8,
    pub fila: u64,
    pub capa: u64,
    pub bytes: u64,
    pub formato: u32,
    pub interno: bool,
}

/// Una caja en texeles: `[x0, y0, z0, x1, y1, z1]` (el fin, sin incluir).
pub(crate) type Caja = [u32; 6];

/// **Mover** la `caja` del subrecurso `sub` de `t` y la memoria `l` (que
/// empieza en el primer elemento de la caja). `hacia`: de `l` a la textura.
///
/// # Safety
/// `l.p` vale para `l.bytes` bytes (leer, y escribir si no es `hacia`).
pub(crate) unsafe fn mover(t: &Tex, sub: u32, caja: Caja, l: &Lineal, hacia: bool) -> Result<(), &'static str> {
    let s = *t.subs.get(sub as usize).ok_or("un subrecurso que la textura no tiene")?;
    let (bi, lado) = t.almacen.elemento();
    let (bn, lado_n) = if l.interno { (bi, lado) } else { crate::d3d12_medidas::elemento(l.formato) };
    if lado != lado_n {
        return Err("copiar entre un formato de bloques y uno que no lo es");
    }
    let [x0, y0, z0, x1, y1, z1] = caja;
    let (c0, c1) = ((x0 as u64) / lado, (x1 as u64).div_ceil(lado));
    let (f0, f1) = ((y0 as u64) / lado, (y1 as u64).div_ceil(lado));
    if x0 >= x1 || y0 >= y1 || z0 >= z1 || c1 * bi > s.fila || f1 > s.filas as u64 || z1 > s.hondo {
        return Err("una caja que sale del subrecurso");
    }
    let (cols, filas, hondo) = (c1 - c0, f1 - f0, (z1 - z0) as u64);
    let ultimo = (hondo - 1) * l.capa + (filas - 1) * l.fila + cols * bn;
    if ultimo > l.bytes {
        return Err("la memoria de la copia es mas corta que la caja");
    }
    let crudo = l.interno || matches!(t.almacen, Almacen::Bloques(_));
    // N5.16: los de float, texel a texel en su formato nativo, ida y vuelta.
    let flotantes = !crudo && t.almacen == Almacen::Flotantes4;
    if !hacia && !crudo && !flotantes && (bn != 4 || !interno_es_nativo(t.forma.formato)) {
        return Err("leer de vuelta una textura que la casa guarda convertida (no es RGBA8, BGRA8, R32, D32 ni BC): todavia no");
    }
    let rebanada = s.fila * s.filas as u64;
    for z in 0..hondo {
        for f in 0..filas {
            let int = (t.datos + s.desde + (z0 as u64 + z) * rebanada + (f0 + f) * s.fila + c0 * bi) as *mut u8;
            let lin = l.p.add((z * l.capa + f * l.fila) as usize);
            if flotantes {
                let nativo = Almacen::nativo(l.formato);
                for c in 0..cols as usize {
                    let (lin_c, int_c) = (lin.add(c * bn as usize), int.add(16 * c));
                    if hacia {
                        let v = bmo_proton_x::formato_ia::leer(nativo, core::slice::from_raw_parts(lin_c, bn as usize));
                        for (k, x) in v.iter().enumerate() {
                            (int_c.add(4 * k) as *mut u32).write_unaligned(x.to_bits());
                        }
                    } else {
                        let v: [u32; 4] = core::array::from_fn(|k| (int_c.add(4 * k) as *const u32).read_unaligned());
                        // 05-10: uno de enteros, sus bits tal cual (los de un float, convertidos).
                        let Some(e) = bmo_proton_x::formato_ia::empaquetar(nativo, v, bmo_proton_x::formato_ia::es_entero(nativo)) else {
                            return Err("leer de vuelta una textura de float en un formato que la casa aun no escribe");
                        };
                        core::ptr::copy_nonoverlapping(e.as_ptr(), lin_c, (bn as usize).min(e.len()));
                    }
                }
            } else if crudo || (!hacia && bn == 4) {
                let n = (cols * bi) as usize;
                if hacia {
                    core::ptr::copy(lin, int, n);
                } else {
                    core::ptr::copy(int, lin, n);
                }
            } else {
                for c in 0..cols as usize {
                    let texel = core::slice::from_raw_parts(lin.add(c * bn as usize), bn as usize);
                    (int.add(4 * c) as *mut u32).write_unaligned(a_interno(l.formato, texel));
                }
            }
        }
    }
    Ok(())
}

/// La caja entera de un subrecurso.
pub(crate) fn caja_entera(t: &Tex, sub: u32) -> Option<Caja> {
    let s = t.subs.get(sub as usize)?;
    Some([0, 0, 0, s.ancho, s.alto, s.hondo])
}

/// La memoria interna de la caja de un subrecurso de `t`, como [`Lineal`]
/// (para copiar de una textura a otra).
pub(crate) fn lineal_interna(t: &Tex, sub: u32, caja: Caja) -> Option<Lineal> {
    let s = t.subs.get(sub as usize)?;
    let (bi, lado) = t.almacen.elemento();
    let rebanada = s.fila * s.filas as u64;
    let desde = s.desde + caja[2] as u64 * rebanada + (caja[1] as u64 / lado) * s.fila + (caja[0] as u64 / lado) * bi;
    Some(Lineal { p: (t.datos + desde) as *mut u8, fila: s.fila, capa: rebanada, bytes: s.bytes().saturating_sub(desde - s.desde), formato: t.forma.formato, interno: true })
}

/// **Una `D3D12_TEXTURE_COPY_LOCATION`**, leida: el recurso, y su
/// subrecurso (Type 0) o su huella en un bufer (Type 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Ubicacion {
    Sub { recurso: u64, sub: u32 },
    Huella { recurso: u64, desde: u64, formato: u32, ancho: u32, alto: u32, hondo: u32, paso: u32 },
}

impl Ubicacion {
    /// pResource +0, Type +8; y +16 el indice o la huella (Offset +16,
    /// Format +24, Width +28, Height +32, Depth +36, RowPitch +40).
    ///
    /// # Safety
    /// `p` son los 48 bytes de una D3D12_TEXTURE_COPY_LOCATION del `.exe`.
    pub(crate) unsafe fn de(p: *const u8) -> Option<Ubicacion> {
        let u32_ = |o: usize| (p.add(o) as *const u32).read_unaligned();
        let u64_ = |o: usize| (p.add(o) as *const u64).read_unaligned();
        match u32_(8) {
            0 => Some(Ubicacion::Sub { recurso: u64_(0), sub: u32_(16) }),
            1 => Some(Ubicacion::Huella { recurso: u64_(0), desde: u64_(16), formato: u32_(24), ancho: u32_(28), alto: u32_(32), hondo: u32_(36).max(1), paso: u32_(40) }),
            _ => None,
        }
    }
}

/// **Una copia de CopyTextureRegion, apuntada**: a donde, en que punto, de
/// donde, y la caja del origen (`None`: todo).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Region {
    pub destino: Ubicacion,
    pub en: [u32; 3],
    pub origen: Ubicacion,
    pub caja: Option<Caja>,
}

fn tex(r: u64) -> Option<&'static Tex> {
    // SAFETY: lo que un `.exe` da a CopyTextureRegion son Recursos de la casa.
    unsafe { de::<Recurso>(r) }.tex.as_ref()
}

fn bufer(r: u64) -> Option<(u64, u64)> {
    // SAFETY: como arriba.
    unsafe { de::<Recurso>(r) }.bufer.as_ref().map(|b| (b.base(), b.bytes as u64))
}

/// La memoria de una huella de un bufer, desde la esquina `(x, y, z)` (en
/// texeles) de la huella.
fn lineal_de_huella(u: Ubicacion, x: u32, y: u32, z: u32) -> Option<Lineal> {
    let Ubicacion::Huella { recurso, desde, formato, alto, paso, .. } = u else { return None };
    let (base, bytes) = bufer(recurso)?;
    let (bn, lado) = crate::d3d12_medidas::elemento(formato);
    let capa = paso as u64 * (alto as u64).div_ceil(lado);
    let o = desde + z as u64 * capa + (y as u64 / lado) * paso as u64 + (x as u64 / lado) * bn;
    (o <= bytes).then(|| Lineal { p: (base + o) as *mut u8, fila: paso as u64, capa, bytes: bytes - o, formato, interno: false })
}

/// **Hacer una copia apuntada** (en ExecuteCommandLists, como la GPU).
pub(crate) fn hacer(c: &Region) -> Result<(), &'static str> {
    let [dx, dy, dz] = c.en;
    match (c.destino, c.origen) {
        // Un bufer a una textura (subir: UpdateSubresources de d3dx12).
        (Ubicacion::Sub { recurso, sub }, h @ Ubicacion::Huella { ancho, alto, hondo, .. }) => {
            let t = tex(recurso).ok_or("CopyTextureRegion a algo que no es una textura")?;
            let [x0, y0, z0, x1, y1, z1] = c.caja.unwrap_or([0, 0, 0, ancho, alto, hondo]);
            let l = lineal_de_huella(h, x0, y0, z0).ok_or("CopyTextureRegion desde una huella que no es de un bufer de la casa")?;
            crate::tuberia::aplicar_limpieza(recurso);
            let caja = [dx, dy, dz, dx + (x1 - x0), dy + (y1 - y0), dz + (z1 - z0)];
            // 05-10: un subrecurso del plano de stencil (`d3d12_stencil`).
            if let Some(p) = (sub as usize).checked_sub(t.subs.len()) {
                // SAFETY: como abajo.
                return unsafe { crate::d3d12_stencil::mover_plano(recurso, p as u32, caja, &l, true) };
            }
            // SAFETY: `l` es memoria de un bufer de la casa, `l.bytes` de largo.
            unsafe { mover(t, sub, caja, &l, true) }
        }
        // Una textura a un bufer (leer: READBACK).
        (h @ Ubicacion::Huella { .. }, Ubicacion::Sub { recurso, sub }) => {
            let t = tex(recurso).ok_or("CopyTextureRegion desde algo que no es una textura")?;
            // 05-10: el plano de stencil: el subrecurso `sub - mips * capas`.
            let plano = (sub as usize).checked_sub(t.subs.len()).map(|p| p as u32);
            let caja = match (c.caja, plano) {
                (Some(k), _) => k,
                (None, Some(p)) => t.subs.get(p as usize).map(|s| [0, 0, 0, s.ancho, s.alto, 1]).ok_or("un subrecurso que la textura no tiene")?,
                (None, None) => caja_entera(t, sub).ok_or("un subrecurso que la textura no tiene")?,
            };
            let l = lineal_de_huella(h, dx, dy, dz).ok_or("CopyTextureRegion a una huella que no es de un bufer de la casa")?;
            crate::tuberia::aplicar_limpieza(recurso);
            if let Some(p) = plano {
                // SAFETY: como abajo.
                return unsafe { crate::d3d12_stencil::mover_plano(recurso, p, caja, &l, false) };
            }
            // SAFETY: como arriba.
            unsafe { mover(t, sub, caja, &l, false) }
        }
        // De una textura a otra.
        (Ubicacion::Sub { recurso: rd, sub: sd }, Ubicacion::Sub { recurso: rs, sub: ss }) => {
            let (td, ts) = (tex(rd).ok_or("CopyTextureRegion a algo que no es una textura")?, tex(rs).ok_or("CopyTextureRegion desde algo que no es una textura")?);
            if td.almacen.elemento() != ts.almacen.elemento() || matches!(td.almacen, Almacen::Bloques(_)) != matches!(ts.almacen, Almacen::Bloques(_)) {
                return Err("CopyTextureRegion entre texturas de formatos que no se copian tal cual");
            }
            let caja = match c.caja {
                Some(k) => k,
                None => caja_entera(ts, ss).ok_or("un subrecurso que la textura no tiene")?,
            };
            let l = lineal_interna(ts, ss, caja).ok_or("un subrecurso que la textura no tiene")?;
            crate::tuberia::aplicar_limpieza(rs);
            crate::tuberia::aplicar_limpieza(rd);
            let [x0, y0, z0, x1, y1, z1] = caja;
            // SAFETY: `l` es memoria interna de una textura de la casa.
            unsafe { mover(td, sd, [dx, dy, dz, dx + (x1 - x0), dy + (y1 - y0), dz + (z1 - z0)], &l, true) }
        }
        _ => Err("CopyTextureRegion de un bufer a otro: en Windows es un error"),
    }
}

/// **`WriteToSubresource`/`ReadFromSubresource` de una textura**: la caja
/// (`D3D12_BOX`: left, top, front, right, bottom, back; nula, todo) entre el
/// subrecurso y la memoria del `.exe` (filas de `paso`, rebanadas de `capa`).
///
/// # Safety
/// `p` es memoria del `.exe` que cubre la caja con esos pasos.
pub(crate) unsafe fn con_el_exe(this: u64, sub: u32, b: *const u8, p: *mut u8, paso: u32, capa: u32, escribir: bool) -> Result<(), &'static str> {
    let t = tex(this).ok_or("WriteToSubresource/ReadFromSubresource de algo que no es una textura")?;
    let caja = if b.is_null() {
        caja_entera(t, sub).ok_or("un subrecurso que la textura no tiene")?
    } else {
        core::array::from_fn(|k| (b.add(4 * k) as *const u32).read_unaligned())
    };
    let filas = (caja[4] - caja[1].min(caja[4])).max(1) as u64;
    let hondo = (caja[5] - caja[2].min(caja[5])).max(1) as u64;
    let bytes = (hondo - 1) * capa as u64 + filas * paso as u64;
    let l = Lineal { p, fila: paso as u64, capa: capa as u64, bytes, formato: t.forma.formato, interno: false };
    crate::tuberia::aplicar_limpieza(this);
    mover(t, sub, caja, &l, escribir)
}
