//! **Quien escribe un BEF2.** Coloca las regiones, la tabla de anexos y los
//! dos anexos que solo el puede saber: los requisitos y la firma.
//!
//! [carril]  AMARILLO  lo que escriba mal lo rechaza el lector, no el metal
//! [cuesta]  TAREA     un `.bex` que no carga
//! [riesgo]  ESPEJO    escribe lo que `lector.rs` juzga
//!
//! ** Dos cosas se fabrican aqui y no las pide el llamante, por el mismo
//! motivo que en BEF1: **el dato solo lo tiene el escritor**. Los requisitos
//! (la memoria que ocupara la imagen) y la firma (el hash de lo que se acaba
//! de escribir). Un productor que tenga que acordarse es un productor que un
//! dia no se acuerda.

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use super::*;
use crate::bef::requisitos;
use crate::bef::blake3::blake3_256;

/// Lo que el programa necesita ademas de su memoria: pantalla, audio, lo que
/// declare. Es [`requisitos::Declaracion`] con el motivo en propiedad.
pub struct Requisito {
    pub clase: u16,
    pub unidad: u16,
    pub obligatorio: bool,
    pub cantidad: u64,
    pub motivo: String,
}

/// El motivo de la linea de memoria que el escritor pone el solo. Es lo que
/// [`Escritor::de_imagen`] usa para reconocerla y no arrastrarla.
const MOTIVO_MEMORIA: &str = "codigo, constantes, datos y ceros de la imagen";

/// Escribe una imagen BEF2.
pub struct Escritor {
    banderas: u8,
    xcr0: u64,
    entrada: u32,
    codigo: Vec<u8>,
    constantes: Vec<u8>,
    datos: Vec<u8>,
    ceros: u32,
    relocs: Vec<Reloc>,
    anexos: Vec<(u8, Vec<u8>)>,
    requisitos: Vec<Requisito>,
    alinear_a_pagina: bool,
    /// La firma de AUTOR, si la hay: `(sig 64, pubkey 32)`.
    ed25519: Option<([u8; 64], [u8; 32])>,
}

impl Escritor {
    /// Un programa que se puede lanzar.
    pub fn ejecutable() -> Self {
        Self {
            banderas: EJECUTABLE,
            // x87 y SSE: lo que BMO C e INTI usan para la coma flotante
            // escalar, y lo que el kernel preserva hoy.
            xcr0: XCR0_X87 | XCR0_SSE,
            entrada: 0,
            codigo: Vec::new(),
            constantes: Vec::new(),
            datos: Vec::new(),
            ceros: 0,
            relocs: Vec::new(),
            anexos: Vec::new(),
            requisitos: Vec::new(),
            alinear_a_pagina: false,
            ed25519: None,
        }
    }

    /// Un objeto sin enlazar (`.bo`): lo consume `bmo-enlazar`.
    pub fn objeto() -> Self {
        let mut e = Self::ejecutable();
        e.banderas = OBJETO;
        e
    }

    pub fn codigo(&mut self, bytes: Vec<u8>) -> &mut Self {
        self.codigo = bytes;
        self
    }

    pub fn constantes(&mut self, bytes: Vec<u8>) -> &mut Self {
        self.constantes = bytes;
        self
    }

    pub fn datos(&mut self, bytes: Vec<u8>) -> &mut Self {
        self.datos = bytes;
        self
    }

    pub fn ceros(&mut self, bytes: u32) -> &mut Self {
        self.ceros = bytes;
        self
    }

    pub fn entrada(&mut self, offset: u32) -> &mut Self {
        self.entrada = offset;
        self
    }

    /// Lo pone el COMPILADOR al ver la operacion, no el autor.
    pub fn quiere_pantalla(&mut self) -> &mut Self {
        self.banderas |= QUIERE_PANTALLA;
        self
    }

    pub fn reloc(&mut self, r: Reloc) -> &mut Self {
        self.relocs.push(r);
        self
    }

    /// Un anexo para OTRO (recursos, manifiesto, katanas, simbolos).
    /// Los relocs, la firma y los requisitos los pone el escritor.
    pub fn anexo(&mut self, tipo: u8, bytes: Vec<u8>) -> &mut Self {
        self.anexos.push((tipo, bytes));
        self
    }

    pub fn requerir(&mut self, r: Requisito) -> &mut Self {
        self.requisitos.push(r);
        self
    }

    /// **Cada region en su propia pagina del FICHERO.** Cuesta relleno y a
    /// cambio el cargador puede REFLEJAR paginas en vez de copiarlas. Sin
    /// medirlo en DOOM no se elige: por eso es una palanca y no una decision
    /// escrita en el formato (ver `docs/plan/PLAN_BEF_NATIVO.md`, decision 2).
    pub fn alinear_a_pagina(&mut self, si: bool) -> &mut Self {
        self.alinear_a_pagina = si;
        self
    }

    /// **Vuelve a abrir una imagen ya escrita** para cambiarle algo y
    /// reescribirla: lo que hacen `bmo-pack` (agregar recursos) y `bmo-firmar`
    /// (poner la firma de autor).
    ///
    /// ** La firma y los relocs se fabrican otra vez al construir, porque
    /// dependen de lo que acabe habiendo dentro. De los requisitos se rehace
    /// SOLO la linea de memoria (la cuenta de ayer); **lo que el programa
    /// DECLARO viaja**: pantalla, audio, lo que sea. La primera version tiraba
    /// la tabla entera, y un `.ibx` que declaraba la pantalla la perdia al
    /// pasar por `bmo-pack` para llevarse su icono -- sin que nadie se
    /// enterara hasta el DIRECTOR. Lo ata `lo_declarado_sobrevive_a_reabrir`.
    pub fn de_imagen(bytes: &[u8]) -> Result<Self, super::Falta> {
        let v = super::leer(bytes)?;
        let mut e = Self::ejecutable();
        e.banderas = v.banderas;
        e.xcr0 = v.xcr0;
        e.entrada = v.entrada;
        e.ceros = v.ceros;
        e.codigo = v.region(Region::Codigo).to_vec();
        e.constantes = v.region(Region::Constantes).to_vec();
        e.datos = v.region(Region::Datos).to_vec();
        for r in v.relocs() {
            e.relocs.push(r);
        }
        for a in v.anexos() {
            let desde = a.tramo.offset as usize;
            let cuerpo = &bytes[desde..desde + a.tramo.bytes as usize];
            match a.tipo {
                // Los que fabrica el escritor se rehacen.
                ANEXO_RELOCS | ANEXO_FIRMA => {}
                // De los requisitos, todo menos la linea que pone el escritor.
                ANEXO_REQUISITOS => {
                    if let Some(t) = requisitos::Tabla::abrir(cuerpo) {
                        for r in t.iter() {
                            let motivo = t.motivo(&r);
                            if r.clase == requisitos::CLASE_MEMORIA && motivo == MOTIVO_MEMORIA {
                                continue;
                            }
                            e.requisitos.push(Requisito {
                                clase: r.clase,
                                unidad: r.unidad,
                                obligatorio: r.es_obligatorio(),
                                cantidad: r.cantidad,
                                motivo: String::from(motivo),
                            });
                        }
                    }
                }
                // El resto es data para otro y viaja tal cual.
                _ => e.anexos.push((a.tipo, cuerpo.to_vec())),
            }
        }
        Ok(e)
    }

    /// **La firma de AUTOR.** `sig` es Ed25519 sobre la cadena de hashes de la
    /// imagen (`cadena_de_hashes`), y `pubkey` la clave con la que se
    /// comprueba. Sin esto, un `.bex` solo dice "llego entero".
    pub fn ed25519(&mut self, sig: [u8; 64], pubkey: [u8; 32]) -> &mut Self {
        self.ed25519 = Some((sig, pubkey));
        self
    }

    pub fn construir(&mut self) -> Result<Vec<u8>, &'static str> {
        if self.banderas & EJECUTABLE != 0 && self.codigo.is_empty() {
            return Err("un ejecutable sin codigo");
        }
        if !self.codigo.is_empty() && self.entrada as usize >= self.codigo.len() {
            return Err("la entrada cae fuera del codigo");
        }

        // -- Los anexos, en orden: lo que el kernel lee primero --------------
        let mut anexos: Vec<(u8, Vec<u8>)> = Vec::new();
        if !self.relocs.is_empty() {
            let mut bytes = Vec::with_capacity(self.relocs.len() * RELOC);
            for r in &self.relocs {
                bytes.extend_from_slice(&r.a_bytes());
            }
            anexos.push((ANEXO_RELOCS, bytes));
        }
        anexos.push((ANEXO_REQUISITOS, self.tabla_de_requisitos()?));
        for (tipo, bytes) in self.anexos.drain(..) {
            if matches!(tipo, ANEXO_RELOCS | ANEXO_FIRMA | ANEXO_REQUISITOS) {
                return Err("ese anexo lo pone el escritor");
            }
            anexos.push((tipo, bytes));
        }
        let idx_firma = anexos.len();
        let cuantos = anexos.len() + 1; // + la firma
        if cuantos > MAX_ANEXOS {
            return Err("demasiados anexos");
        }

        // -- La firma: el INDICE primero, un hash por region con bytes, y uno
        //    por CADA anexo (2026-09-20: antes solo los que el kernel lee, y
        //    el indice ninguno). Se sabe CUANTOS antes de colocar nada, que es
        //    lo que deja calcular el medida del anexo y por tanto los offsets.
        let mut cubre: Vec<u8> = vec![FIRMA_INDICE];
        for (i, r) in [&self.codigo, &self.constantes, &self.datos].iter().enumerate() {
            if !r.is_empty() {
                cubre.push(i as u8);
            }
        }
        for i in 0..anexos.len() {
            cubre.push(FIRMA_ANEXO | i as u8);
        }
        let firma_bytes = FIRMA_CABECERA
            + cubre.len() * FIRMA_HASH
            + if self.ed25519.is_some() { FIRMA_ED25519 } else { 0 };

        // -- La colocacion ---------------------------------------------------
        //
        // ** LAS REGIONES EMPIEZAN EN UN SECTOR (512) desde B6 (2026-09-19).
        // El cargador del kernel pide cada region al disco por rangos y el HBA
        // escribe los sectores ENTEROS directamente en el marco del proceso;
        // lo que empieza a mitad de sector pasa por el sector de rebote y se
        // copia (`bmo_fat32::leer_en`, "correcto siempre, rapido cuando el
        // formato ayuda"). BEF1 alineaba a 512 desde el 10-08 por esto mismo y
        // la primera version de BEF2 lo perdio (16): cada region empezaba con
        // una cabeza rebotada. Con pagina (`alinear_a_pagina`, la decision B9)
        // se puede ademas REFLEJAR en vez de copiar.
        let paso = if self.alinear_a_pagina { 4096 } else { 512 };
        let mut cursor = CABECERA + cuantos * ANEXO;
        let tramo = |datos: &[u8], cursor: &mut usize, paso: usize| -> (u32, u32) {
            if datos.is_empty() {
                return (0, 0);
            }
            *cursor = (*cursor + paso - 1) / paso * paso;
            let off = *cursor;
            *cursor += datos.len();
            (off as u32, datos.len() as u32)
        };
        let t_codigo = tramo(&self.codigo, &mut cursor, paso);
        let t_constantes = tramo(&self.constantes, &mut cursor, paso);
        let t_datos = tramo(&self.datos, &mut cursor, paso);

        // ** LO QUE EL KERNEL LEE VA PRIMERO EN EL FICHERO, y la firma detras
        // de eso; el resto (recursos, manifiesto, simbolos...) al final. Asi
        // `hasta_donde_hace_falta` acaba en la firma y un paquete con un WAD
        // de seis megas se ejecuta leyendo ochocientos kilos. La primera version
        // ponia la firma la ULTIMA de todas, detras de los recursos, y la
        // puerta tenia que pedir el fichero entero para llegar a los hashes.
        // (El indice de la tabla no cambia: la firma sigue siendo la ultima
        // ENTRADA, y `que` nombra por entrada, no por posicion.)
        let mut sitio_anexo: Vec<(u32, u32)> = vec![(0, 0); anexos.len()];
        for (i, (_, bytes)) in anexos.iter().enumerate().filter(|(_, (t, _))| lo_lee_el_kernel(*t)) {
            cursor = (cursor + 7) / 8 * 8;
            sitio_anexo[i] = (cursor as u32, bytes.len() as u32);
            cursor += bytes.len();
        }
        cursor = (cursor + 7) / 8 * 8;
        let sitio_firma = (cursor as u32, firma_bytes as u32);
        cursor += firma_bytes;
        for (i, (_, bytes)) in anexos.iter().enumerate().filter(|(_, (t, _))| !lo_lee_el_kernel(*t)) {
            cursor = (cursor + 7) / 8 * 8;
            sitio_anexo[i] = (cursor as u32, bytes.len() as u32);
            cursor += bytes.len();
        }
        let total = cursor;
        if total > u32::MAX as usize {
            return Err("la imagen no cabe en 4 GiB");
        }

        // -- Los bytes -------------------------------------------------------
        let mut img = vec![0u8; total];
        img[0..4].copy_from_slice(&MAGIC.to_le_bytes());
        img[4] = ABI;
        img[5] = self.banderas;
        img[8..16].copy_from_slice(&self.xcr0.to_le_bytes());
        img[16..20].copy_from_slice(&self.entrada.to_le_bytes());
        img[20..24].copy_from_slice(&(cuantos as u32).to_le_bytes());
        for (o, t) in [(24, t_codigo), (32, t_constantes), (40, t_datos)] {
            img[o..o + 4].copy_from_slice(&t.0.to_le_bytes());
            img[o + 4..o + 8].copy_from_slice(&t.1.to_le_bytes());
        }
        img[48..52].copy_from_slice(&self.ceros.to_le_bytes());
        img[52..56].copy_from_slice(&(total as u32).to_le_bytes());

        for (i, ((tipo, bytes), (off, len))) in anexos.iter().zip(sitio_anexo.iter()).enumerate() {
            let e = CABECERA + i * ANEXO;
            img[e] = *tipo;
            img[e + 4..e + 8].copy_from_slice(&off.to_le_bytes());
            img[e + 8..e + 12].copy_from_slice(&len.to_le_bytes());
            let o = *off as usize;
            img[o..o + bytes.len()].copy_from_slice(bytes);
        }
        let e = CABECERA + idx_firma * ANEXO;
        img[e] = ANEXO_FIRMA;
        img[e + 4..e + 8].copy_from_slice(&sitio_firma.0.to_le_bytes());
        img[e + 8..e + 12].copy_from_slice(&sitio_firma.1.to_le_bytes());

        for (datos, t) in [
            (&self.codigo, t_codigo),
            (&self.constantes, t_constantes),
            (&self.datos, t_datos),
        ] {
            if t.1 > 0 {
                let o = t.0 as usize;
                img[o..o + datos.len()].copy_from_slice(datos);
            }
        }

        // La firma, al final y sobre los bytes ya puestos: la cabecera y la
        // tabla de anexos estan completas (la entrada de la firma incluida),
        // asi que el hash del indice se calcula sobre lo que el kernel leera.
        let f = sitio_firma.0 as usize;
        img[f..f + 4].copy_from_slice(&(cubre.len() as u32).to_le_bytes());
        let algo = if self.ed25519.is_some() { ALGO_ED25519 } else { ALGO_NINGUNO };
        img[f + 4..f + 8].copy_from_slice(&algo.to_le_bytes());
        for (i, que) in cubre.iter().enumerate() {
            let h = f + FIRMA_CABECERA + i * FIRMA_HASH;
            img[h] = *que;
            let trozo: &[u8] = if *que == FIRMA_INDICE {
                &img[..CABECERA + cuantos * ANEXO]
            } else if que & FIRMA_ANEXO != 0 {
                let (off, len) = sitio_anexo[(que & !FIRMA_ANEXO) as usize];
                &img[off as usize..off as usize + len as usize]
            } else {
                let t = [t_codigo, t_constantes, t_datos][*que as usize];
                &img[t.0 as usize..t.0 as usize + t.1 as usize]
            };
            let digest = blake3_256(trozo);
            img[h + 8..h + FIRMA_HASH].copy_from_slice(&digest);
        }
        if let Some((sig, pubkey)) = self.ed25519 {
            let s = f + FIRMA_CABECERA + cubre.len() * FIRMA_HASH;
            img[s..s + 64].copy_from_slice(&sig);
            img[s + 64..s + 96].copy_from_slice(&pubkey);
        }
        Ok(img)
    }

    /// **La memoria que ocupara la imagen**, que el escritor acaba de colocar y
    /// nadie mas sabe, mas lo que el programa haya declarado.
    fn tabla_de_requisitos(&self) -> Result<Vec<u8>, &'static str> {
        let memoria = self.codigo.len() as u64
            + self.constantes.len() as u64
            + self.datos.len() as u64
            + self.ceros as u64;
        let mut decls = vec![requisitos::Declaracion {
            clase: requisitos::CLASE_MEMORIA,
            unidad: requisitos::UNIDAD_BYTES,
            obligatorio: true,
            cantidad: memoria,
            motivo: MOTIVO_MEMORIA,
        }];
        for r in self.requisitos.iter() {
            decls.push(requisitos::Declaracion {
                clase: r.clase,
                unidad: r.unidad,
                obligatorio: r.obligatorio,
                cantidad: r.cantidad,
                motivo: &r.motivo,
            });
        }
        requisitos::construir(&decls)
    }
}
