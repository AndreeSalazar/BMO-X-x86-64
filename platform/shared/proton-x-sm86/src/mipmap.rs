//! **G5: LA MIP LA ELIGE EL SOMBREADOR** (13-10, la puerta `[3]`) -- un
//! `SampleLevel` o un `Load` de una 2D con varias mips, en la 3060, con las
//! MISMAS cuentas que la casa (`Textura::elegir`, `muestrear_lod`, `cargar`).
//! Cada mip es un TIC PITCH de un nivel leido de la RAM del juego
//! (`bmo_gpu_ga10x::mipmap`): no hay mips en el TIC, asi que el `.LL` de la
//! 3060 no las elegiria; las elige esto, y lee cada una con `.LZ`.
//!
//! capa: puro -- instrucciones y numeros; la textura la presta el kernel
//!
//! [eje]     CORRECCION -- los bits de la casa: la mip, el peso y la mezcla
//!           con sus mismas operaciones de f32 (sin FFMA: la casa redondea dos
//!           veces)
//!
//! # Las dos filas de cada textura (las escribe la PUERTA, cada lote)
//!
//! ```text
//!    fila 0   sesgo   el MipLODBias del muestreador (f32)
//!             lo, hi  la sujecion ENTERA de la casa (`sujetar`: MinLOD,
//!                     MaxLOD, ResourceMinLODClamp y las mips de la vista)
//!                     como UN clamp -- una cadena de max y min con
//!                     constantes es max(min(l, hi), lo) con lo y hi la
//!                     cadena en -inf y en +inf --
//!             c       128.0 con la mip PUNTO, 0.5 con LINEAL (abajo)
//!    fila 1   vista   la mip que mira la vista (entera)
//!             peso    255 con LINEAL, 0 con PUNTO (entera)
//! ```
//!
//! # Un `SampleLevel(s, uv, nivel)`
//!
//! ```text
//!    l  = max(min(nivel + sesgo, ...)): FADD, FMNMX lo, FMNMX hi
//!         (un NaN: FMNMX da lo, como la casa, que lo lleva a -inf)
//!    F  = F2I.U32.TRUNC(l * 256 + c)            l >= 0: TRUNC es suelo
//!    k  = F >> 8 + vista ;  f = F & 255 & peso
//!         LINEAL: `fijo(l)` de la casa, suelo(l * 256 + 0.5)
//!         PUNTO:  suelo(l + 0.5) = suelo(256 (l + 0.5)) >> 8, y 256 (l + 0.5)
//!                 es EXACTAMENTE l * 256 + 128 en f32 (escalar por 2^8 no
//!                 cambia el redondeo)
//!    q1 = TEX.LZ(hueco 1 + min(k, 14))          la mip k
//!    q2 = TEX.LZ(hueco 1 + min(k + min(f, 1), 14))
//!    d  = q1 + (q2 - q1) * (I2F(f) * 1/256)     la casa: x + (y - x) * f
//! ```
//!
//! Con `f = 0` (PUNTO, o la ultima mip) `q2` es `q1` (la misma lectura) y la
//! mezcla da `q1` bit a bit. `l <= hi <= mips - 1`, asi que `k` no pasa de
//! la ultima, y con `l == hi` entero, `f = 0`.
//!
//! # Un `Load(x, y, mip)`
//!
//! `TLD.LZ(hueco 1 + min(mip + vista, 14))`: la casa da ceros con
//! `vista + mip` fuera de las mips del RECURSO; aqui un negativo (sin signo,
//! enorme) o uno de mas caen en el hueco 15 o en uno sin mip, los dos la
//! textura NULA.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use bmo_gpu_ga10x::mipmap as gm;
use bmo_gpu_ga10x::texturas::{DeApp, Formato, Muestreo};

use bmo_proton_x::dxil::programa::Reg;
use bmo_proton_x::textura::{Filtro, Muestreador, Textura};
use bmo_sm86::codifica::{self as c, DimTex, Fuente, NivelTex};

use super::planifica::Meta;
use super::{reg_de, Clase, Emisor, NoEmite, Precarga};

/// Filas de constantes por textura, detras del cbuffer.
pub const FILAS: u32 = 2;
/// `256.0f32`, `1.0 / 256.0`.
const DOS_56: u32 = 0x4380_0000;
const UN_256AVO: u32 = 0x3B80_0000;
/// `c` de cada filtro de mip.
pub const C_PUNTO: f32 = 128.0;
pub const C_LINEAL: f32 = 0.5;

/// Si la textura `t` de `p` es una `Texture2D` (la PSV0, `Ranuras::forma`):
/// las que eligen su mip aqui.
pub fn es_2d(p: &bmo_proton_x::dxil::programa::Programa, t: u8) -> bool {
    p.ranuras.texturas.get(t as usize).and_then(|l| p.ranuras.forma(*l)) == Some(2)
}

/// **Las dos filas de (tN, sM)** para este lote, con la textura y el
/// muestreador de la casa: las cuentas de `Textura::sujetar` y `elegir`.
pub fn filas(t: &Textura, m: &Muestreador) -> [[u32; 4]; 2] {
    let ultima = (t.niveles_vista() - 1) as f32;
    let piso = (t.lod_min - t.mip as f32).max(0.0);
    let g = |l: f32| l.max(m.lod.minimo).min(m.lod.maximo).max(piso).min(ultima);
    let (lo, hi) = (g(f32::NEG_INFINITY), g(f32::INFINITY));
    let lineal = m.lod.mip == Filtro::Lineal;
    let c = if lineal { C_LINEAL } else { C_PUNTO };
    [[m.lod.sesgo.to_bits(), lo.to_bits(), hi.to_bits(), c.to_bits()], [t.mip, if lineal { 255 } else { 0 }, 0, 0]]
}

/// **Una textura del lote para la receta** (G5): con `elige` (el de
/// pixeles elige su mip), su CADENA entera desde la mip 0 de la capa de la
/// vista; si no, el plano de la vista, como antes. Lo que no cae en 32 B --
/// la COLA -- se pone con filas de 32 detras de lo que ya hay en `datos`.
pub(crate) fn de_app(t: &Textura, muestreo: Muestreo, elige: bool, datos: &mut Vec<u8>, tn: u8) -> Result<DeApp, String> {
    let Some(formato) = formato_de(t) else {
        return Err(format!("la textura t{tn} es de float, BC6 o BC4/BC5 con signo: la 3060 todavia lee RGBA/BGRA de 8 bits y BC1-5 y BC7"));
    };
    let (texeles, ancho, alto, mips, vista) = if elige {
        let Some((cadena, mips)) = t.cadena() else {
            return Err(format!("la textura t{tn}: sus mips no estan en la memoria que dio la casa"));
        };
        if mips > gm::MAX_MIPS {
            return Err(format!("la textura t{tn} tiene {mips} mips: la 3060 lleva {} por textura", gm::MAX_MIPS));
        }
        (cadena, t.ancho, t.alto, mips, t.mip)
    } else {
        let Some((plano, ancho, alto)) = t.plano_visto() else {
            return Err(format!("la textura t{tn}: su mip {} no esta en la memoria que dio la casa", t.mip));
        };
        (plano, ancho, alto, 1, 0)
    };
    let fila = gm::fila_natural(ancho, formato, 0);
    let mut d = DeApp { va: texeles.as_ptr() as u64, ancho, alto, fila, mips, vista, cola_en: 0, formato, srgb: t.srgb, muestreo };
    if (texeles.len() as u64) * 4 < gm::desde(ancho, alto, fila, formato, mips) {
        return Err(format!("la textura t{tn} ({ancho}x{alto}, {mips} mips) no cabe en la memoria que dio la casa"));
    }
    // La cola: cada mip desde la primera que no cae en 32 B, sus filas (de
    // texeles o de bloques) a 32.
    let cola = d.cola();
    if cola < mips {
        let en = datos.len().next_multiple_of(32);
        let n = d.bytes_cola() as usize;
        if n > gm::MAX_COLA as usize {
            return Err(format!("la textura t{tn}: sus mips de filas que no caen en 32 B miden {n} bytes (la cola lleva {})", gm::MAX_COLA));
        }
        datos.resize(en + n, 0);
        let byte = |b: usize| (texeles[b / 4] >> (8 * (b % 4))) as u8;
        for j in cola..mips {
            let (de, a) = (gm::desde(ancho, alto, fila, formato, j) as usize, en + gm::en_cola(ancho, alto, formato, cola, j) as usize);
            let (natural, en_cola) = (gm::fila_natural(ancho, formato, j) as usize, gm::fila_en_cola(ancho, formato, j) as usize);
            for y in 0..gm::filas(alto, formato, j) as usize {
                for x in 0..natural {
                    datos[a + y * en_cola + x] = byte(de + y * natural + x);
                }
            }
        }
        d.cola_en = en as u32;
    }
    if !d.valida() {
        return Err(format!("la textura t{tn} ({ancho}x{alto}, {mips} mips, VA {:#x}): no cabe en su ranura de {} MiB", d.va, bmo_gpu_ga10x::texturas::RANURA >> 20));
    }
    Ok(d)
}

/// **El formato de la 3060 de una textura de la casa** (13-10: tambien los
/// BC, que la 3060 descomprime ella misma); `None` lo que todavia no.
pub(crate) fn formato_de(t: &Textura) -> Option<Formato> {
    use bmo_proton_x::bc::Bc;
    use bmo_proton_x::textura::Como;
    Some(match t.como {
        Como::Rgba8 => Formato::Rgba8,
        Como::Bgra8 => Formato::Bgra8,
        Como::Bloques(Bc::Bc1) => Formato::Bc1,
        Como::Bloques(Bc::Bc2) => Formato::Bc2,
        Como::Bloques(Bc::Bc3) => Formato::Bc3,
        Como::Bloques(Bc::Bc4) => Formato::Bc4,
        Como::Bloques(Bc::Bc5) => Formato::Bc5,
        Como::Bloques(Bc::Bc7) => Formato::Bc7,
        _ => return None,
    })
}

/// **Las filas de la textura `k`** en los DATOS (detras de las `filas_cb`
/// del cbuffer: [`FILAS`] por textura).
pub(crate) fn poner_filas(datos: &mut [u8], filas_cb: u32, k: usize, f: &[[u32; 4]; 2]) {
    for (r, fila) in f.iter().enumerate() {
        let o = 16 * (filas_cb as usize + FILAS as usize * k + r);
        for (j, w) in fila.iter().enumerate() {
            datos[o + 4 * j..o + 4 * j + 4].copy_from_slice(&w.to_le_bytes());
        }
    }
}

/// ** G5: lee el de pixeles con nivel (o un Load) alguna textura que NO
/// elige su mip aqui (no es 2D): esas siguen con la mip 0 del TIC.
pub fn nivel_sin_mips(p: &bmo_proton_x::dxil::programa::Programa) -> bool {
    use bmo_proton_x::dxil::programa::{Lectura, Op};
    p.ops.iter().any(|o| matches!(*o, Op::Lee { t, como: Lectura::Nivel | Lectura::Carga { .. }, .. } if !es_2d(p, t)))
}

impl Emisor<'_> {
    fn base_de(&mut self, t: u8, s: u8) -> Result<u8, NoEmite> {
        self.precarga(Precarga::Base { textura: t, muestreador: s, reg: 0 })
    }

    fn fila_de(&mut self, t: u8, s: u8, fila: u8) -> Result<u8, NoEmite> {
        self.precarga(Precarga::Mips { textura: t, muestreador: s, fila, reg: 0 })
    }

    /// `x = base + min(k, TOPE)`: el asa de la mip `k`, en la forma que deja
    /// el juez R7 (`juez_asas`). `x` y el sujeto, de paso.
    fn asa_de_mip(&mut self, base: u8, k: u8, paso: &mut Vec<u8>) -> Result<u8, NoEmite> {
        let kc = self.pedir()?;
        paso.push(kc);
        self.poner(c::imnmx(kc, k, Fuente::Imm(bmo_gpu_ga10x::mipmap::TOPE), false, false, 0), Clase::Alu, Some(kc), [Some(k), None, None]);
        let h = self.pedir()?;
        paso.push(h);
        self.poner(c::iadd3(h, base, c::r(kc), 0), Clase::Alu, Some(h), [Some(base), Some(kc), None]);
        Ok(h)
    }

    /// Un TEX o TLD `.LZ` 2D de los cuatro canales en `q` (alineado).
    fn leer_lz(&mut self, q: u8, ra: u8, h: u8, carga: bool) {
        let w = if carga { c::tld(q, ra, h, DimTex::D2, NivelTex::Cero, 0) } else { c::tex_forma(q, ra, h, DimTex::D2, NivelTex::Cero, 0) };
        self.poner_meta(w, Meta { escribe_n: 4, lee_tarde: true, ..Meta::de(Clase::Tex, Some(q), [Some(ra), Some(ra + 1), Some(h)]) });
    }

    /// **`d..d+4 = tN.SampleLevel(sM, uv, nivel)` o `tN.Load(xy, nivel)`**
    /// de una 2D, con sus mips (ver el modulo). `ra` las coordenadas, ya en
    /// su par.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn lee_mips(&mut self, d: Reg, t: u8, s: u8, carga: bool, ra: u8, nivel: Reg, paso: &mut Vec<u8>) -> Result<(), NoEmite> {
        let base = self.base_de(t, s)?;
        let m1 = self.fila_de(t, s, 1)?;
        let fuente_nivel = self.fuente(nivel);
        let q = self.bloque(4, false)?;
        if carga {
            // La mip de la vista mas la pedida (enteros).
            let k = self.pedir()?;
            paso.push(k);
            self.poner(c::iadd3(k, m1, fuente_nivel, 0), Clase::Alu, Some(k), [Some(m1), reg_de(fuente_nivel), None]);
            let h = self.asa_de_mip(base, k, paso)?;
            self.leer_lz(q, ra, h, true);
        } else {
            let m0 = self.fila_de(t, s, 0)?;
            let l = self.pedir()?;
            paso.push(l);
            // nivel + sesgo (conmuta: el sesgo es registro), sujeto.
            self.poner(c::fadd(l, c::r(m0), fuente_nivel, false, 0), Clase::Fma, Some(l), [Some(m0), reg_de(fuente_nivel), None]);
            self.poner(c::fmnmx(l, c::r(l), c::r(m0 + 1), true, 0), Clase::Alu, Some(l), [Some(l), Some(m0 + 1), None]);
            self.poner(c::fmnmx(l, c::r(l), c::r(m0 + 2), false, 0), Clase::Alu, Some(l), [Some(l), Some(m0 + 2), None]);
            self.poner(c::fmul(l, c::r(l), Fuente::Imm(DOS_56), false, 0), Clase::Fma, Some(l), [Some(l), None, None]);
            self.poner(c::fadd(l, c::r(l), c::r(m0 + 3), false, 0), Clase::Fma, Some(l), [Some(l), Some(m0 + 3), None]);
            // F, y de el k (con la vista) y f (con el peso).
            self.poner(c::f2i(l, l, false, 0), Clase::Mufu, Some(l), [Some(l), None, None]);
            self.mufus += 1;
            let (k, f) = (self.pedir()?, self.pedir()?);
            paso.extend([k, f]);
            self.poner(c::shr(k, l, Fuente::Imm(8), false, 0), Clase::Alu, Some(k), [Some(l), None, None]);
            self.poner(c::lop3(f, l, Fuente::Imm(255), c::Y, 0), Clase::Alu, Some(f), [Some(l), None, None]);
            self.poner(c::lop3(f, f, c::r(m1 + 1), c::Y, 0), Clase::Alu, Some(f), [Some(f), Some(m1 + 1), None]);
            self.poner(c::iadd3(k, k, c::r(m1), 0), Clase::Alu, Some(k), [Some(k), Some(m1), None]);
            // La primera mip.
            let h = self.asa_de_mip(base, k, paso)?;
            self.leer_lz(q, ra, h, false);
            // La segunda: k + min(f, 1) (con f = 0, la misma).
            let uno = self.pedir()?;
            paso.push(uno);
            self.poner(c::imnmx(uno, f, Fuente::Imm(1), false, false, 0), Clase::Alu, Some(uno), [Some(f), None, None]);
            self.poner(c::iadd3(uno, k, c::r(uno), 0), Clase::Alu, Some(uno), [Some(k), Some(uno), None]);
            let h2 = self.asa_de_mip(base, uno, paso)?;
            let q2 = self.bloque(4, false)?;
            paso.extend(q2..q2 + 4);
            self.leer_lz(q2, ra, h2, false);
            // El peso, f / 256 (exacto), y la mezcla de la casa canal a canal.
            self.poner(c::i2f(f, f, false, 0), Clase::Mufu, Some(f), [Some(f), None, None]);
            self.mufus += 1;
            self.poner(c::fmul(f, c::r(f), Fuente::Imm(UN_256AVO), false, 0), Clase::Fma, Some(f), [Some(f), None, None]);
            for j in 0..4u8 {
                let (x, y) = (q + j, q2 + j);
                self.poner(c::fadd(y, c::r(y), c::neg(x), false, 0), Clase::Fma, Some(y), [Some(y), Some(x), None]);
                self.poner(c::fmul(y, c::r(y), c::r(f), false, 0), Clase::Fma, Some(y), [Some(y), Some(f), None]);
                self.poner(c::fadd(x, c::r(x), c::r(y), false, 0), Clase::Fma, Some(x), [Some(x), Some(y), None]);
            }
        }
        for j in 0..4u8 {
            self.valor[d as usize + j as usize] = Some(super::Valor::Reg(q + j));
        }
        Ok(())
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use bmo_proton_x::textura::{Clase as Cl, Como, Direccion, Lod};
    use bmo_gpu_ga10x::mipmap as gm;

    fn tex(t: &[u32], mips: u32, mip: u32) -> Textura<'_> {
        Textura { texeles: t, ancho: 16, alto: 8, como: Como::Rgba8, srgb: false, mapeo: Textura::MAPEO, mips, capas: 1, hondo: 1, clase: Cl::Plana, mip, capa: 0, niveles: u32::MAX, lod_min: 0.0, vista: None }
    }

    /// 13-10: un BC1 de 64x32 con sus 7 mips va a la 3060 como esta: las
    /// mips de filas de bloques de 32 B en la RAM de la app; desde la de 8x4
    /// (dos bloques de 8 B por fila) a la cola, con sus filas a 32.
    #[test]
    fn un_bc1_con_sus_mips() {
        use bmo_proton_x::bc::Bc;
        let palabras = [16 * 8, 8 * 4, 4 * 2, 2, 1, 1, 1].iter().sum::<usize>() * 2;
        let todo: alloc::vec::Vec<u32> = (0..palabras as u32 + 8).map(|k| k.wrapping_mul(0x9E37_79B9)).collect();
        // Desde 32 B, como un bloque de la casa (los del juego, en pagina).
        let salto = (32 - todo.as_ptr() as usize % 32) % 32 / 4;
        let t = &todo[salto..salto + palabras];
        let tx = Textura { como: Como::Bloques(Bc::Bc1), mips: 7, ..tex(t, 7, 0) };
        let tx = Textura { ancho: 64, alto: 32, ..tx };
        let m = Muestreo { lineal: true, u: 1, v: 1, borde: [0.0; 4] };
        let mut datos = alloc::vec![0u8; 48];
        let d = de_app(&tx, m, true, &mut datos, 0).expect("va");
        assert_eq!((d.formato, d.fila, d.mips), (Formato::Bc1, 128, 7));
        // La 2 (16x8: 4 bloques, 32 B) aun en la RAM; la 3 (8x4) a la cola.
        assert_eq!(d.cola(), 3);
        assert_eq!(d.cola_en, 64, "la cola, tras lo que habia, en 32");
        let byte = |b: usize| (t[b / 4] >> (8 * (b % 4))) as u8;
        let de = gm::desde(64, 32, 128, Formato::Bc1, 3) as usize;
        assert_eq!(&datos[64..80], &(0..16).map(|k| byte(de + k)).collect::<alloc::vec::Vec<u8>>()[..], "la de 8x4: sus dos bloques");
        assert!(datos[80..96].iter().all(|&b| b == 0), "y el relleno de su fila");
        assert_eq!(datos.len(), 64 + 32 * 4, "la 3, 4, 5 y 6: una fila de 32 cada una");
    }

    #[test]
    fn las_filas_son_la_sujecion_de_la_casa() {
        let t = [0u32; 16 * 8 * 2];
        let m = Muestreador { filtro: Filtro::Lineal, u: Direccion::Repetir, v: Direccion::Repetir, borde: [0.0; 4], comparacion: 0, lod: Lod { min: Some(Filtro::Lineal), mip: Filtro::Lineal, sesgo: 0.5, minimo: 0.25, maximo: 2.5 } };
        let f = filas(&tex(&t, 5, 1), &m);
        // La vista desde la mip 1 ve 4: hasta 3. MaxLOD 2.5 manda.
        assert_eq!(f[0].map(f32::from_bits), [0.5, 0.25, 2.5, C_LINEAL]);
        assert_eq!(f[1], [1, 255, 0, 0]);
        let p = Muestreador { lod: Lod { mip: Filtro::Punto, maximo: f32::MAX, ..m.lod }, ..m };
        let f = filas(&tex(&t, 5, 3), &p);
        assert_eq!(f[0].map(f32::from_bits), [0.5, 0.25, 1.0, C_PUNTO]);
        assert_eq!(f[1], [3, 0, 0, 0]);
    }
}
