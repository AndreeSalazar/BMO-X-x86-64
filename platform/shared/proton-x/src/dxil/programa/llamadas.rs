//! **Las llamadas a `dx.op.*`** de `programa.rs` (05-10): el DESPACHADOR,
//! movido tal cual para que `programa.rs` no pase de las 1.000 lineas (L6a).
//!
//! Es hijo de `programa`: ve todo lo suyo con `use super::*`. Lo unico que
//! cambio al moverlo: `super::` es ahora `super::super::` (la carpeta `dxil`
//! sigue siendo la misma) y la funcion es `pub(super)`.

use super::*;

/// Una llamada a `dx.op.*`: el primer argumento es el numero de operacion.
pub(super) fn llamada(c: &mut Compilador, args: &[usize], nombre: &str) -> Result<Valor, NoPrograma> {
    // E6c: lo que trae ENTEROS lo dice su sobrecarga (`dx.op.X.i32`).
    let enteros = nombre.ends_with(".i32");
    let arg = |k: usize| args.get(k).copied().ok_or(NoPrograma::Forma("una operacion de D3D con menos argumentos"));
    let op = c.entero(arg(0)?)?;
    let uno = |c: &mut Compilador, f: fn(Reg, Reg) -> Op| -> Result<Valor, NoPrograma> {
        let a = c.float(arg(1)?)?;
        let d = c.registro(0.0)?;
        c.ops.push(f(d, a));
        Ok(Valor::Float(d))
    };
    // Las olas (E2.5: de verdad) y las derivadas: `olas.rs`.
    if let Some(v) = super::super::olas::de(c, op, args, nombre) {
        return v;
    }
    // Gather y SampleCmp (las sombras): `sombras.rs`.
    if let Some(v) = super::super::sombras::de(c, op, args) {
        return v;
    }
    Ok(match op {
        DX_LOAD_INPUT | DX_STORE_OUTPUT => {
            if !matches!(c.valores.get(arg(2)?), Some(Valor::Entero(_))) {
                return Err(NoPrograma::Forma("loadInput/storeOutput con una fila CALCULADA (una entrada en array): todavia no"));
            }
            let (elemento, fila, col) = (c.entero(arg(1)?)?, c.entero(arg(2)?)?, c.entero(arg(3)?)?);
            if fila != 0 || !(0..32).contains(&elemento) || !(0..4).contains(&col) {
                return Err(NoPrograma::Forma("una entrada o salida en array o fuera de rango: todavia no"));
            }
            let (elemento, componente) = (elemento as u8, col as u8);
            if op == DX_LOAD_INPUT {
                c.entradas = c.entradas.max(elemento as usize + 1);
                c.lee |= 1 << elemento;
                let d = c.registro(0.0)?;
                // E2.3b: en un GS, el quinto es el VERTICE de la primitiva
                // (`input[k]`); en los demas, `undef`.
                match c.valores.get(arg(4)?) {
                    Some(Valor::Entero(k)) if (0..6).contains(k) => c.ops.push(Op::EntradaDe { d, vertice: *k as u8, elemento, componente }),
                    Some(Valor::Entero(_)) => return Err(NoPrograma::Forma("un GS que lee un vertice que su primitiva no tiene")),
                    Some(Valor::Indefinido) | None => c.ops.push(Op::Entrada { d, elemento, componente }),
                    _ => return Err(NoPrograma::Forma("un GS que lee la entrada de un vertice CALCULADO: todavia no")),
                }
                if enteros {
                    Valor::Bits(d)
                } else {
                    Valor::Float(d)
                }
            } else {
                c.salidas = c.salidas.max(elemento as usize + 1);
                let s = super::super::estructura::bits(c, arg(4)?)?;
                c.ops.push(Op::Salida { s, elemento, componente });
                Valor::Nada
            }
        }
        // N5.5: los ids del hilo de computo.
        DX_THREAD_ID | DX_GROUP_ID | DX_THREAD_ID_IN_GROUP | DX_FLATTENED_THREAD_ID_IN_GROUP => {
            let que = (op - DX_THREAD_ID) as u8;
            let comp = if op == DX_FLATTENED_THREAD_ID_IN_GROUP { 0 } else { c.entero(arg(1)?)? };
            if !(0..3).contains(&comp) {
                return Err(NoPrograma::Forma("un id de hilo con un componente que no es x, y ni z"));
            }
            let d = c.registro(0.0)?;
            c.ops.push(Op::IdHilo { d, que, c: comp as u8 });
            Valor::Bits(d)
        }
        // A6 (06-10): `primitiveID()`, el SV_PrimitiveID de un GS: un id
        // mas (`que` 4), que da el GS que corre (`Tiras::primitiva`).
        DX_PRIMITIVE_ID => {
            let d = c.registro(0.0)?;
            c.ops.push(Op::IdHilo { d, que: 4, c: 0 });
            Valor::Bits(d)
        }
        // N5.5: la barrera del grupo (con cualquier modo: la de la memoria
        // del grupo, la de los UAV, o las dos; todas esperan a todos).
        DX_BARRIER => {
            c.ops.push(Op::Barrera);
            Valor::Nada
        }
        // N5.5: `bufferStore(uav, coord0, coord1, v0, v1, v2, v3, mascara)`.
        DX_BUFFER_STORE => {
            let Some(Valor::Uav(u, modo)) = c.valores.get(arg(1)?).copied() else {
                return Err(NoPrograma::Forma("BufferStore sin el handle de un UAV de bufer"));
            };
            let cero = super::super::estructura::literal(c, 0)?;
            let i = super::super::estructura::bits(c, arg(2)?)?;
            let desp = if matches!(c.valores.get(arg(3)?), Some(Valor::Indefinido) | None) { cero } else { super::super::estructura::bits(c, arg(3)?)? };
            let mut v = [cero; 4];
            for (k, r) in v.iter_mut().enumerate() {
                if !matches!(c.valores.get(arg(4 + k)?), Some(Valor::Indefinido) | None) {
                    *r = super::super::estructura::bits(c, arg(4 + k)?)?;
                }
            }
            let mascara = c.entero(arg(8)?)? as u8;
            c.ops.push(Op::EscribeUav { u, modo, i, desp, z: cero, v, mascara });
            Valor::Nada
        }
        // N5.3c: `textureStore(uav, coord0, coord1, coord2, v0..v3, mascara)`:
        // el texel (x, y) de un RWTexture2D y (06-10) la z de un RWTexture3D
        // o la capa de un RWTexture2DArray.
        DX_TEXTURE_STORE => {
            let Some(Valor::Uav(u, crate::bufer::Modo::Textura)) = c.valores.get(arg(1)?).copied() else {
                return Err(NoPrograma::Forma("TextureStore sin el handle de un UAV de textura"));
            };
            let cero = super::super::estructura::literal(c, 0)?;
            let (i, desp) = (super::super::estructura::bits(c, arg(2)?)?, super::super::estructura::bits(c, arg(3)?)?);
            let z = coordenada(c, arg(4)?, cero)?;
            let mut v = [cero; 4];
            for (k, r) in v.iter_mut().enumerate() {
                if !matches!(c.valores.get(arg(5 + k)?), Some(Valor::Indefinido) | None) {
                    *r = super::super::estructura::bits(c, arg(5 + k)?)?;
                }
            }
            let mascara = c.entero(arg(9)?)? as u8;
            c.ops.push(Op::EscribeUav { u, modo: crate::bufer::Modo::Textura, i, desp, z, v, mascara });
            Valor::Nada
        }
        // E2.4: `bufferUpdateCounter(uav, inc)`.
        DX_BUFFER_UPDATE_COUNTER => {
            let Some(Valor::Uav(u, _)) = c.valores.get(arg(1)?).copied() else {
                return Err(NoPrograma::Forma("BufferUpdateCounter sin el handle de un UAV de bufer"));
            };
            let inc = c.entero(arg(2)?)?;
            if inc != 1 && inc != -1 {
                return Err(NoPrograma::Forma("BufferUpdateCounter con un paso que no es 1 ni -1"));
            }
            let d = c.registro(0.0)?;
            c.ops.push(Op::Contador { d, u, inc: inc as i8 });
            Valor::Bits(d)
        }
        // 05-10: `atomicBinOp(uav, op, c0, c1, c2, v)` y
        // `atomicCompareExchange(uav, c0, c1, c2, igual, v)`: las coordenadas
        // como las de un bufferStore (c1, el desplazamiento o la y; 06-10, la
        // c2, la z de una textura 3D o la capa de un array).
        DX_ATOMIC_BIN_OP | DX_ATOMIC_COMPARE_EXCHANGE => {
            let Some(Valor::Uav(u, modo)) = c.valores.get(arg(1)?).copied() else {
                return Err(NoPrograma::Forma("un Interlocked sin el handle de un UAV"));
            };
            let binaria = op == DX_ATOMIC_BIN_OP;
            let como = if binaria { crate::bufer::Atomo::de_dxil(c.entero(arg(2)?)?).ok_or(NoPrograma::Forma("un atomicBinOp con una operacion que no existe"))? } else { crate::bufer::Atomo::CambiaSiIgual };
            let c0 = if binaria { 3 } else { 2 };
            let cero = super::super::estructura::literal(c, 0)?;
            let i = super::super::estructura::bits(c, arg(c0)?)?;
            let desp = if matches!(c.valores.get(arg(c0 + 1)?), Some(Valor::Indefinido) | None) { cero } else { super::super::estructura::bits(c, arg(c0 + 1)?)? };
            let z = coordenada(c, arg(c0 + 2)?, cero)?;
            let v = super::super::estructura::bits(c, arg(6)?)?;
            let igual = if binaria { cero } else { super::super::estructura::bits(c, arg(5)?)? };
            let d = c.registro(0.0)?;
            c.ops.push(Op::Atomico { d, u, modo, i, desp, z, como, v, igual });
            Valor::Bits(d)
        }
        // E2.3b: el GS emite un vertice, corta la tira, o las dos.
        DX_EMIT_STREAM | DX_CUT_STREAM | DX_EMIT_THEN_CUT_STREAM => {
            let flujo = c.entero(arg(1)?)?;
            if !(0..4).contains(&flujo) {
                return Err(NoPrograma::Forma("un GS con un flujo que no es 0..3"));
            }
            if op != DX_CUT_STREAM {
                c.ops.push(Op::Emite { flujo: flujo as u8 });
            }
            if op != DX_EMIT_STREAM {
                c.ops.push(Op::Corta { flujo: flujo as u8 });
            }
            Valor::Nada
        }
        // N5.7: `discard(i1 c)`; `clip(x)` llega como `discard(x < 0)`, y un
        // `discard` a secas, con un `i1 true` (un literal).
        DX_DISCARD => {
            let c_ = super::super::estructura::bits(c, arg(1)?)?;
            c.ops.push(Op::Descarta { c: c_ });
            Valor::Nada
        }
        DX_CREATE_HANDLE => {
            // (clase, rango, indice, no uniforme): 0 SRV, 1 UAV, 2 CBuffer, 3
            // Sampler. El indice es el REGISTRO (la base del rango incluida);
            // el ESPACIO, el del rango `rango` de su clase en la PSV0 (03-10).
            if !matches!(c.valores.get(arg(3)?), Some(Valor::Entero(_))) {
                // N5.4 (05-10): el registro CALCULADO. Las texturas, si; un
                // array de buferes, de cbuffers, de muestreadores o de UAV,
                // todavia no, y se dice cual.
                let (clase, rango) = (c.entero(arg(1)?)?, c.entero(arg(2)?)?);
                let Some(r) = (rango >= 0).then(|| super::super::recursos::rango(&c.recursos, clase as u8, rango as u32)).flatten() else {
                    return Err(NoPrograma::Forma("createHandle con un registro calculado de un rango que la PSV0 no declara"));
                };
                return match clase {
                    0 if r.modo_de_bufer().is_none() => {
                        let i = super::super::estructura::bits(c, arg(3)?)?;
                        Ok(Valor::TexturaEn { rango: c.ranuras.dinamica(r.espacio, r.desde)?, i })
                    }
                    0 => Err(NoPrograma::Forma("createHandle con un registro CALCULADO de un array de BUFERES: todavia no (N5.4)")),
                    2 => Err(NoPrograma::Forma("createHandle con un registro CALCULADO de un array de CBUFFERS: todavia no (N5.4)")),
                    3 => Err(NoPrograma::Forma("createHandle con un registro CALCULADO de un array de MUESTREADORES: todavia no (N5.4)")),
                    _ => Err(NoPrograma::Forma("createHandle con un registro CALCULADO de un array de UAV: todavia no (N5.4)")),
                };
            }
            let (clase, rango, indice) = (c.entero(arg(1)?)?, c.entero(arg(2)?)?, c.entero(arg(3)?)?);
            if !(0..=3).contains(&clase) || rango < 0 || indice < 0 {
                return Err(NoPrograma::Forma("un createHandle con una clase, un rango o un registro imposibles"));
            }
            let espacio = super::super::recursos::rango(&c.recursos, clase as u8, rango as u32).map_or(0, |r| r.espacio);
            let registro = indice as u32;
            match clase {
                2 => Valor::Cbuffer(c.ranuras.cbuffer(espacio, registro)?),
                0 => {
                    let t = c.ranuras.textura(espacio, registro)?;
                    match super::super::recursos::rango(&c.recursos, 0, rango as u32).and_then(|r| r.modo_de_bufer()) {
                        Some(modo) => Valor::Bufer(t, modo),
                        None => Valor::Textura(t),
                    }
                }
                3 => Valor::Muestreador(c.ranuras.muestreador(espacio, registro)?),
                // N5.5: los UAV de BUFER (RWStructuredBuffer, RWByteAddress
                // Buffer, RWBuffer); N5.3c (05-10), los de TEXTURA de una o
                // dos dimensiones (RWTexture1D, RWTexture2D); y (06-10) los
                // 3D y los arrays de 2D (RWTexture3D, RWTexture2DArray: la z)
                // y (A5) de 1D (RWTexture1DArray, 6: la capa es la y; lo sabe
                // su vista, `bufer::Rebanadas::una_d`).
                1 => match super::super::recursos::rango(&c.recursos, 1, rango as u32).map(|r| (r.modo_de_bufer(), r.especie)) {
                    Some((Some(modo), _)) => Valor::Uav(c.ranuras.uav(espacio, registro)?, modo),
                    Some((None, 1 | 2 | 4 | 6 | 7)) => Valor::Uav(c.ranuras.uav(espacio, registro)?, crate::bufer::Modo::Textura),
                    _ => return Err(NoPrograma::Forma("un UAV de TEXTURA multimuestra o de cubo: todavia no")),
                },
                _ => return Err(NoPrograma::Forma("un createHandle de una clase que no existe")),
            }
        }
        DX_CBUFFER_LOAD_LEGACY => {
            let Some(&Valor::Cbuffer(cb)) = c.valores.get(arg(1)?) else {
                return Err(NoPrograma::Forma("CBufferLoadLegacy sin el handle del cbuffer"));
            };
            // 03-10: la fila CALCULADA (un array del cbuffer).
            if !matches!(c.valores.get(arg(2)?), Some(Valor::Entero(_))) {
                let i = super::super::estructura::bits(c, arg(2)?)?;
                let d = c.registro(0.0)?;
                for _ in 0..3 {
                    c.registro(0.0)?;
                }
                c.filas_cb = c.filas_cb.max(FILAS_DE_D3D);
                c.ops.push(Op::ConstantesEn { d, fila: 0, filas: FILAS_DE_D3D, i, cb });
                return Ok(if enteros { Valor::CuatroEnteros(d) } else { Valor::Cuatro(d) });
            }
            let fila = c.entero(arg(2)?)?;
            if !(0..4096).contains(&fila) {
                return Err(NoPrograma::Forma("CBufferLoadLegacy con una fila fuera del cbuffer"));
            }
            let d = c.registro(0.0)?;
            for _ in 0..3 {
                c.registro(0.0)?;
            }
            c.filas_cb = c.filas_cb.max(fila as u16 + 1);
            c.ops.push(Op::Constantes { d, fila: fila as u16, cb });
            if enteros { Valor::CuatroEnteros(d) } else { Valor::Cuatro(d) }
        }
        DX_SAMPLE | DX_SAMPLE_BIAS | DX_SAMPLE_LEVEL | DX_SAMPLE_GRAD => {
            // (srv, sampler, coord0..3, offset0..2, y lo de cada una: el
            // sesgo, la mip o los gradientes, y el clamp).
            let (Some(t), Some(Valor::Muestreador(sm))) = (textura(c, arg(1)?), c.valores.get(arg(2)?).copied()) else {
                return Err(NoPrograma::Forma("Sample sin el handle de una textura y el de un muestreador"));
            };
            let desp = desplazamientos(c, [arg(7)?, arg(8)?, arg(9)?])?;
            let indefinido = |k: usize| matches!(c.valores.get(k), Some(Valor::Indefinido) | None);
            let plana = indefinido(arg(5)?) && indefinido(arg(6)?) && desp == [0; 3];
            // D4.4: el clamp del sombreador (la mip mas detallada que deja),
            // el ultimo argumento de Sample, SampleBias y SampleGrad.
            let clamp = match op {
                DX_SAMPLE => Some(10),
                DX_SAMPLE_BIAS => Some(11),
                DX_SAMPLE_GRAD => Some(16),
                _ => None,
            };
            let sin_clamp = clamp.is_none_or(|k| args.get(k).is_none_or(|&a| indefinido(a)));
            // La ELEGIDA va siempre por `Lee`: `Muestra` es lo que sabe la
            // 3060, y la 3060 no elige texturas (todavia).
            if op == DX_SAMPLE && plana && sin_clamp && t != DINAMICA {
                // Lo de siempre (2D, sin desplazar): lo que sabe la 3060. D4.4:
                // con la mip de los gradientes de (u, v) en su cuadro.
                let (u, v) = (c.float(arg(3)?)?, c.float(arg(4)?)?);
                let g = cuatro(c)?;
                super::super::olas::gradientes(&mut c.ops, g, u, v);
                let d = cuatro(c)?;
                c.ops.push(Op::Muestra { d, t, s: sm, u, v, g: Some(g) });
                Valor::Cuatro(d)
            } else {
                let co = [super::super::estructura::bits(c, arg(3)?)?, super::super::estructura::bits(c, arg(4)?)?, super::super::estructura::bits(c, arg(5)?)?, super::super::estructura::bits(c, arg(6)?)?];
                let (como, nivel) = match op {
                    DX_SAMPLE_LEVEL => (Lectura::Nivel, super::super::estructura::bits(c, arg(10)?)?),
                    // D4.4: SampleGrad con los suyos (ddx 10, 11; ddy 13, 14);
                    // Sample y SampleBias (su sesgo, el 10), con los del cuadro.
                    _ => {
                        let grad = if op == DX_SAMPLE_GRAD { Some([arg(10)?, arg(11)?, arg(13)?, arg(14)?]) } else { None };
                        let sesgo = if op == DX_SAMPLE_BIAS { Some(arg(10)?) } else { None };
                        (Lectura::Gradientes { compara: false }, super::super::olas::bloque(c, co, grad, &[sesgo, clamp.and_then(|k| args.get(k).copied())])?)
                    }
                };
                let d = cuatro(c)?;
                c.ops.push(Op::Lee { d, t, s: sm, como, c: co, nivel, desp });
                Valor::Cuatro(d)
            }
        }
        DX_TEXTURE_LOAD => {
            // N5.3c: de un UAV de textura: el texel (x, y), sin mip; y (06-10)
            // la z de un 3D o la capa de un array.
            if let Some(Valor::Uav(u, modo @ crate::bufer::Modo::Textura)) = c.valores.get(arg(1)?).copied() {
                let (i, desp) = (super::super::estructura::bits(c, arg(3)?)?, super::super::estructura::bits(c, arg(4)?)?);
                let cero = super::super::estructura::literal(c, 0)?;
                let z = coordenada(c, arg(5)?, cero)?;
                let d = cuatro(c)?;
                c.ops.push(Op::LeeUav { d, u, modo, i, desp, z });
                return Ok(if enteros { Valor::CuatroEnteros(d) } else { Valor::Cuatro(d) });
            }
            // (srv, mip o muestra, coord0..2, offset0..2).
            let Some(t) = textura(c, arg(1)?) else {
                return Err(NoPrograma::Forma("TextureLoad sin el handle de una textura (un UAV o un bufer: todavia no)"));
            };
            let nivel = super::super::estructura::bits(c, arg(2)?)?;
            let co = [super::super::estructura::bits(c, arg(3)?)?, super::super::estructura::bits(c, arg(4)?)?, super::super::estructura::bits(c, arg(5)?)?, super::super::estructura::literal(c, 0)?];
            let desp = desplazamientos(c, [arg(6)?, arg(7)?, arg(8)?])?;
            let d = cuatro(c)?;
            c.ops.push(Op::Lee { d, t, s: 0, como: Lectura::Carga { enteros }, c: co, nivel, desp });
            if enteros { Valor::CuatroEnteros(d) } else { Valor::Cuatro(d) }
        }
        DX_BUFFER_LOAD => {
            // (srv, indice, desplazamiento): el desplazamiento solo lo trae
            // uno estructurado; en los demas es `undef`.
            // N5.5: de un UAV (RWStructuredBuffer leido), por su lado.
            if let Some(Valor::Uav(u, modo)) = c.valores.get(arg(1)?).copied() {
                let cero = super::super::estructura::literal(c, 0)?;
                let i = super::super::estructura::bits(c, arg(2)?)?;
                let desp = if matches!(c.valores.get(arg(3)?), Some(Valor::Indefinido) | None) { cero } else { super::super::estructura::bits(c, arg(3)?)? };
                let d = cuatro(c)?;
                c.ops.push(Op::LeeUav { d, u, modo, i, desp, z: cero });
                return Ok(if enteros { Valor::CuatroEnteros(d) } else { Valor::Cuatro(d) });
            }
            let Some(Valor::Bufer(t, modo)) = c.valores.get(arg(1)?).copied() else {
                return Err(NoPrograma::Forma("BufferLoad sin el handle de un bufer"));
            };
            let cero = super::super::estructura::literal(c, 0)?;
            let indice = super::super::estructura::bits(c, arg(2)?)?;
            let desp = if matches!(c.valores.get(arg(3)?), Some(Valor::Indefinido) | None) { cero } else { super::super::estructura::bits(c, arg(3)?)? };
            let d = cuatro(c)?;
            c.ops.push(Op::Lee { d, t, s: 0, como: Lectura::Bufer(modo), c: [indice, desp, cero, cero], nivel: cero, desp: [0; 3] });
            if enteros { Valor::CuatroEnteros(d) } else { Valor::Cuatro(d) }
        }
        // D4.4: `calculateLOD(srv, sampler, c0, c1, c2, sujeta)`: el LOD de
        // los gradientes de (c0, c1) en su cuadro, en un float.
        DX_CALCULATE_LOD => {
            let (Some(t), Some(Valor::Muestreador(sm))) = (textura(c, arg(1)?), c.valores.get(arg(2)?).copied()) else {
                return Err(NoPrograma::Forma("CalculateLevelOfDetail sin el handle de una textura y el de un muestreador"));
            };
            let sujeta = c.entero(arg(6)?)? != 0;
            let cero = super::super::estructura::literal(c, 0)?;
            let co = [super::super::estructura::bits(c, arg(3)?)?, super::super::estructura::bits(c, arg(4)?)?, super::super::estructura::bits(c, arg(5)?)?, cero];
            let nivel = super::super::olas::bloque(c, co, None, &[])?;
            let d = cuatro(c)?;
            c.ops.push(Op::Lee { d, t, s: sm, como: Lectura::Lod { sujeta }, c: co, nivel, desp: [0; 3] });
            Valor::Float(d)
        }
        DX_GET_DIMENSIONS => {
            // (handle, mip): %dx.types.Dimensions, cuatro i32. De un bufer
            // (N5.3), sus elementos; el mip es `undef`.
            let t = match c.valores.get(arg(1)?).copied() {
                Some(Valor::Textura(_) | Valor::TexturaEn { .. }) => textura(c, arg(1)?).ok_or(NoPrograma::Forma("GetDimensions sin textura"))?,
                Some(Valor::Bufer(t, modo)) => {
                    let cero = super::super::estructura::literal(c, 0)?;
                    let d = cuatro(c)?;
                    c.ops.push(Op::Lee { d, t, s: 0, como: Lectura::MedidasBufer(modo), c: [cero; 4], nivel: cero, desp: [0; 3] });
                    return Ok(Valor::CuatroEnteros(d));
                }
                // N5.3c: de un UAV.
                Some(Valor::Uav(u, modo)) => {
                    let d = cuatro(c)?;
                    c.ops.push(Op::MedidasUav { d, u, modo });
                    return Ok(Valor::CuatroEnteros(d));
                }
                _ => return Err(NoPrograma::Forma("GetDimensions de algo que no es una textura, un bufer ni un UAV")),
            };
            let nivel = super::super::estructura::bits(c, arg(2)?)?;
            let cero = super::super::estructura::literal(c, 0)?;
            let d = cuatro(c)?;
            c.ops.push(Op::Lee { d, t, s: 0, como: Lectura::Medidas, c: [cero; 4], nivel, desp: [0; 3] });
            Valor::CuatroEnteros(d)
        }
        DX_FMAD => {
            let (a, b, cc) = (c.float(arg(1)?)?, c.float(arg(2)?)?, c.float(arg(3)?)?);
            let d = c.registro(0.0)?;
            c.ops.push(Op::Mad { d, a, b, c: cc });
            Valor::Float(d)
        }
        DX_DOT2 | DX_DOT3 | DX_DOT4 => {
            let n = (op - DX_DOT2 + 2) as usize;
            let (mut a, mut b) = ([0 as Reg; 4], [0 as Reg; 4]);
            for k in 0..n {
                a[k] = c.float(arg(1 + k)?)?;
                b[k] = c.float(arg(1 + n + k)?)?;
            }
            let d = c.registro(0.0)?;
            c.ops.push(Op::Dot { d, n: n as u8, a, b });
            Valor::Float(d)
        }
        // N5.6: la matematica (`crate::mates`). f16tof32 lee un entero; f32tof16
        // da uno (sus bits); las demas, float a float.
        _ if crate::mates::Mate::de_dxil(op).is_some() => {
            let f = crate::mates::Mate::de_dxil(op).unwrap_or(crate::mates::Mate::Frac);
            let a = if f.lee_entero() { super::super::estructura::bits(c, arg(1)?)? } else { c.float(arg(1)?)? };
            let d = c.registro(0.0)?;
            c.ops.push(Op::Mate { d, a, f });
            if f.da_entero() {
                Valor::Bits(d)
            } else if f.da_booleano() {
                Valor::Bool(d)
            } else {
                Valor::Float(d)
            }
        }
        DX_RSQRT => uno(c, |d, a| Op::Rsqrt { d, a })?,
        DX_SQRT => uno(c, |d, a| Op::Sqrt { d, a })?,
        DX_SATURATE => uno(c, |d, a| Op::Saturate { d, a })?,
        DX_FABS => uno(c, |d, a| Op::Abs { d, a })?,
        // E6c: IMax, IMin, UMax, UMin (`dx.op.binary.i32`).
        37..=40 => super::super::enteros::min_max(c, op, arg(1)?, arg(2)?)?,
        DX_FMIN | DX_FMAX => {
            let (a, b) = (c.float(arg(1)?)?, c.float(arg(2)?)?);
            let d = c.registro(0.0)?;
            c.ops.push(if op == DX_FMIN { Op::Min { d, a, b } } else { Op::Max { d, a, b } });
            Valor::Float(d)
        }
        otra => return Err(NoPrograma::OperacionD3d(otra)),
    })
}

/// 06-10: la tercera coordenada de un UAV de textura (la z de un 3D, la
/// capa de un array): sus bits, o `cero` si no viene (`undef`, en uno 2D).
fn coordenada(c: &mut Compilador, id: usize, cero: Reg) -> Result<Reg, NoPrograma> {
    if matches!(c.valores.get(id), Some(Valor::Indefinido) | None) {
        Ok(cero)
    } else {
        super::super::estructura::bits(c, id)
    }
}
