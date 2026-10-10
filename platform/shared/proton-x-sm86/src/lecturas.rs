//! **LAS LECTURAS DE TEXTURA CON NIVEL, Y EL LOAD, EN LA 3060** (E8g de
//! `docs/plan/PLAN_LA_LENGUA_DE_LA_3060.md`, DL17 del 09-10): `Op::Lee` con
//! [`Lectura::Nivel`] (`SampleLevel`) y [`Lectura::Carga`] (`Load`), por la
//! forma de TEX o TLD de su textura -- que dice la PSV0 (`Ranuras::forma`)
//! --, con lo que dijo `ptxas` (`bmo_sm86::codifica::ORO_TEX`):
//!
//! ```text
//!    especie         lectura   la 3060              Ra..                Rb, Rb+1
//!    2 Texture2D     Nivel     TEX.SCR.B.LL 2D      u, v                asa, nivel (f32)
//!                    Carga     TLD.SCR.B.LL.CL 2D   x, y                asa, mip (entero)
//!    4 Texture3D     Nivel     TEX.B.LL 3D          u, v, w             asa, nivel
//!    7 Tex2DArray    Nivel     TEX.B.LL ARRAY_2D    capa, u, v          asa, nivel
//!                    Carga     TLD.B.LL.CL ARRAY_2D capa, x, y          asa, mip
//! ```
//!
//! **La capa** de un `SampleLevel` es la de la casa (`Textura::en_su_mip`):
//! `suelo(z + 0.5)`, sujeta a las capas. Aqui: FADD z + 0.5 (el mismo
//! redondeo que la casa), F2I.TRUNC -- en los negativos difiere de `suelo`,
//! pero los dos acaban en 0 al sujetar; un NaN da 0 en los dos --, el mayor
//! con 0 y el menor con 0xFFFF (lo que pone `ptxas`: el campo es de 16 bits).
//! La sujecion de arriba a las capas de la VISTA la hace la 3060 con su TIC
//! (por ver en el metal). En un `Load` la capa ya es entera: sujeta a 0xFFFF,
//! y fuera de la vista la 3060 da 0, como D3D y como la casa (por ver).
//!
//! **El par (asa, nivel)** es de su (textura, muestreador)
//! (`Precarga::AsaPar`, fijo todo el programa: el asa la carga el pegamento
//! UNA vez y el cuerpo no la puede pisar -- R7 --); el nivel se escribe en
//! Rb+1 justo antes de CADA TEX. La 3060 lee las fuentes de un TEX cuando
//! las lee -- despues de salir la instruccion --: el TEX enciende una
//! barrera de LECTURA (`Meta::lee_tarde`) y quien escriba despues en una de
//! ellas -- el nivel de la lectura siguiente del mismo par, o lo que caiga en
//! sus coordenadas -- la espera (lo que el juez mira en R4, y lo que hace
//! NAK). Asi las coordenadas se devuelven en cuanto sale. Con un par y un
//! bloque fijos por lectura, `niveles.hlsl` de dxc no cabia en 64 registros.
//!
//! **Lo que NO va todavia, y por que** (va por la CPU, con su indice):
//! el CUBO -- normalizarlo como `ptxas` con la division EXACTA de la casa
//! son tres divisiones de ~60 instrucciones y la puerta es de 128; con la
//! de la 3060 no serian los bits de la casa: es una decision --; los
//! gradientes (el `Sample` de un pixel: el TEX que saca la mip de su
//! cuadro), los desplazamientos, `GetDimensions`, el Load de una 3D y la
//! textura elegida al correr.

use alloc::vec::Vec;

use bmo_proton_x::dxil::programa::{Lectura, Reg, DINAMICA};
use bmo_sm86::codifica::{self as c, DimTex, Fuente, NivelTex};

use super::planifica::Meta;
use super::{reg_de, Abi, Clase, Emisor, NoEmite};

/// `0.5f32`.
const MEDIO: u32 = 0x3F00_0000;

impl Emisor<'_> {
    /// **`d..d+4 = tN.SampleLevel(sM, c, nivel)` o `tN.Load(c, nivel)`**.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn lee_textura(&mut self, d: Reg, t: u8, s: u8, como: Lectura, co: [Reg; 4], nivel: Reg, desp: [i8; 3], k: usize, paso: &mut Vec<u8>) -> Result<(), NoEmite> {
        let _ = s;
        if self.abi != Abi::Registros || t == DINAMICA || desp != [0; 3] {
            return Err(NoEmite::Operacion(k));
        }
        if (0..4).any(|j| self.valor.get(d as usize + j).is_none_or(|x| x.is_some())) {
            return Err(NoEmite::NoSsa(k));
        }
        let especie = self.p.ranuras.texturas.get(t as usize).and_then(|l| self.p.ranuras.forma(*l));
        let (dim, carga) = match (como, especie) {
            (Lectura::Nivel, Some(2)) => (DimTex::D2, false),
            (Lectura::Nivel, Some(4)) => (DimTex::D3, false),
            (Lectura::Nivel, Some(7)) => (DimTex::Array2D, false),
            (Lectura::Carga { .. }, Some(2)) => (DimTex::D2, true),
            (Lectura::Carga { .. }, Some(7)) => (DimTex::Array2D, true),
            _ => return Err(NoEmite::Operacion(k)),
        };
        let Some(asa) = self.pares.get(k).copied().flatten() else {
            return Err(NoEmite::Operacion(k));
        };
        // El nivel, en Rb+1 (los bits: un f32 en el TEX, un entero en el TLD).
        let f = self.fuente(nivel);
        self.poner(c::mov(asa + 1, f, 0), Clase::Alu, Some(asa + 1), [reg_de(f), None, None]);
        // Las coordenadas, en su bloque (alineado a 2 o a 4), de paso.
        let n = dim.coordenadas();
        let ancho = if n == 2 { 2 } else { 4 };
        let ra = self.bloque(ancho, false)?;
        paso.extend(ra..ra + ancho as u8);
        let (desde, capa) = if dim == DimTex::Array2D { (1, true) } else { (0, false) };
        for j in 0..(n - desde) {
            let f = self.fuente(co[j as usize]);
            let x = ra + desde + j;
            self.poner(c::mov(x, f, 0), Clase::Alu, Some(x), [reg_de(f), None, None]);
        }
        if capa {
            if carga {
                // Ya entera: sujeta a 16 bits.
                let rz = self.registro(co[2], paso)?;
                self.poner(c::imnmx(ra, rz, Fuente::Imm(0xFFFF), false, false, 0), Clase::Alu, Some(ra), [Some(rz), None, None]);
            } else {
                // suelo(z + 0.5), sujeta: FADD, F2I.TRUNC, max 0, min 0xFFFF.
                let rz = self.registro(co[2], paso)?;
                self.poner(c::fadd(ra, c::r(rz), Fuente::Imm(MEDIO), false, 0), Clase::Fma, Some(ra), [Some(rz), None, None]);
                self.poner(c::f2i(ra, ra, true, 0), Clase::Mufu, Some(ra), [Some(ra), None, None]);
                self.mufus += 1;
                self.poner(c::imnmx(ra, ra, Fuente::Imm(0), true, true, 0), Clase::Alu, Some(ra), [Some(ra), None, None]);
                self.poner(c::imnmx(ra, ra, Fuente::Imm(0xFFFF), false, false, 0), Clase::Alu, Some(ra), [Some(ra), None, None]);
            }
        }
        // La lectura: los cuatro canales, seguidos y alineados.
        let q = self.bloque(4, false)?;
        let w = if carga { c::tld(q, ra, asa, dim, NivelTex::De, 0) } else { c::tex_forma(q, ra, asa, dim, NivelTex::De, 0) };
        let mas = [(n > 2).then_some(ra + 2), (n > 3).then_some(ra + 3), Some(asa + 1)];
        self.poner_meta(w, Meta { escribe_n: 4, lee_mas: mas, lee_tarde: true, ..Meta::de(Clase::Tex, Some(q), [Some(ra), Some(ra + 1), Some(asa)]) });
        for j in 0..4u8 {
            self.valor[d as usize + j as usize] = Some(super::Valor::Reg(q + j));
        }
        Ok(())
    }
}
