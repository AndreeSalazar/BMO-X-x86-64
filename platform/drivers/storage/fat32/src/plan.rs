//! **PLANEAR UN TRAMO: donde esta en el disco lo que toca leer, SIN leerlo.**
//! (paso D1 del plan del disco, 2026-09-23)
//!
//! ## Por que existe
//!
//! `leer_tramo` sigue la cadena Y mueve los datos, cluster a cluster, en la
//! misma llamada. Para el HILO DEL DISCO eso no sirve: el hilo tiene que poder
//! mandar UNA orden de datos, soltar el disco y DORMIR hasta que el aparato
//! acabe. Asi que aqui se separa lo que dura poco --seguir la cadena-- de lo
//! que dura mucho --mover los bytes--, y lo segundo lo hace quien llama.
//!
//! ## ** Y el plan NO LEE la FAT (2026-09-23, la segunda vuelta)
//!
//! La primera version seguia la cadena con `read_fat_entry`, "que casi siempre
//! sale de su cache". El casi era el problema: cuando no salia, el hilo del
//! disco leia un sector de la FAT **en sincrono, con las interrupciones
//! cerradas y el cerrojo `disco` en la mano**. El Ryzen lo dijo el 23-09 a las
//! 06:57: `retenido 134 us`, el cerrojo `disco` en `hilo.rs`, justo una lectura
//! de 512 bytes de un SATA. El hilo que existe para no girar sobre el disco
//! giraba sobre el disco.
//!
//! Ahora el plan es una funcion PURA de lo que ya hay en memoria:
//!
//! ```text
//!   planear_tramo_en(.., ventana)  ->  Tramo      lo tenia todo
//!                                  ->  Falta(s)   le falta el sector s de la FAT
//!                                  ->  Nada       no hay nada que leer
//! ```
//!
//! y quien llama trae ese sector **como trae cualquier otro**: el hilo, como
//! una orden en vuelo mas; el camino sincrono de siempre, leyendolo ahi mismo
//! ([`FatVolume::planear_tramo`]). Los dos recorren la cadena con el MISMO
//! codigo (`planear_con`): lo unico que cambia es de donde sale una entrada.
//!
//! ## Lo que devuelve, y el contrato
//!
//! Un [`Tramo`]: los clusters SEGUIDOS en el disco a partir del cursor, juntos
//! en una sola orden. Un fichero escrito de una vez suele estar entero seguido,
//! y entonces 128 KiB son UNA orden en vez de una por cluster.
//!
//! [!] **El LBA del tramo es ABSOLUTO** (ya lleva `part_lba`): lo consume el
//! kernel para programar el AHCI directamente. Es la suma que costo dos tandas
//! de fotos el 10-08 (ver `leer_de_una_particion_que_no_empieza_en_cero`), y
//! por eso tiene su propia prueba con el volumen desplazado. **El sector de
//! `Falta` es RELATIVO al volumen**, el de la FAT: se convierte con
//! [`FatVolume::ventana_fat`], que es quien sabe la suma.
//!
//! [!] **Los sectores cubren el rabo ENTERO**: el ultimo sector se lee completo
//! aunque el fichero acabe a mitad. Quien reciba los datos tiene que tener sitio
//! hasta la frontera de sector -- el bufer de un fichero del kernel va por
//! paginas, asi que lo tiene.

use super::FatVolume;

/// Lo que mas pide una orden: 8.192 sectores = 4 MiB, una entrada de PRDT llena.
pub const SECTORES_MAX: usize = 8192;

/// Un tramo contiguo del fichero, listo para una sola orden de lectura.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Tramo {
    /// Primer sector, ABSOLUTO en el dispositivo.
    pub lba: u64,
    /// Sectores a leer (el ultimo puede traer bytes de mas tras el fin).
    pub sectores: u16,
    /// Bytes utiles del fichero que trae este tramo.
    pub bytes: usize,
    /// Cluster por el que seguir despues. `0` = el fichero (o su cadena) acabo.
    pub siguiente: u32,
}

/// Lo que contesta el plan que no lee.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Plan {
    /// Nada que leer: el fichero acabo, o el cluster no es de este volumen.
    Nada,
    /// El tramo, planeado entero con lo que habia en la ventana.
    Tramo(Tramo),
    /// Para seguir la cadena le falta ESTE sector de la FAT, relativo al
    /// volumen. Quien llama lo trae --a su manera y a su tiempo-- y vuelve a
    /// preguntar desde el mismo cursor.
    Falta(u64),
}

/// **Sectores de la FAT que alguien ya trajo a memoria.** El plan los LEE; no
/// los pide.
#[derive(Clone, Copy)]
pub struct VentanaFat<'a> {
    /// Primer sector de la ventana, RELATIVO al volumen (el mismo de `Falta`).
    pub sector: u64,
    /// Sus bytes, sector detras de sector.
    pub bytes: &'a [u8],
}

impl VentanaFat<'_> {
    /// La que no tiene nada: todo lo que se le pregunte es un `Falta`.
    pub const VACIA: VentanaFat<'static> = VentanaFat { sector: 0, bytes: &[] };

    /// La entrada cruda en `idx` del sector `sector`, si la ventana lo cubre.
    /// Todo con cuentas comprobadas: la ventana puede venir de cualquier sitio.
    fn entrada(&self, sector: u64, idx: usize) -> Option<u32> {
        let k = usize::try_from(sector.checked_sub(self.sector)?).ok()?;
        let off = k.checked_mul(512)?.checked_add(idx)?;
        let b = self.bytes.get(off..off.checked_add(4)?)?;
        Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
}

impl FatVolume {
    /// **El siguiente tramo contiguo, con la FAT que haya en `ventana` y sin
    /// leer nada.** Mismo contrato que [`planear_tramo`](Self::planear_tramo);
    /// si la cadena pide un sector que la ventana no tiene, `Plan::Falta`.
    pub fn planear_tramo_en(
        &self,
        cluster: u32,
        ya: usize,
        file_size: u32,
        tope: usize,
        ventana: &VentanaFat,
    ) -> Plan {
        self.planear_con(cluster, ya, file_size, tope, |s, idx| ventana.entrada(s, idx))
    }

    /// **El siguiente tramo contiguo** a partir de `cluster`, que contiene el
    /// byte `ya` del fichero (frontera de cluster, el mismo contrato que
    /// `leer_tramo`). Junta clusters mientras sean consecutivos en el disco, sin
    /// pasar de `tope` bytes ni de [`SECTORES_MAX`].
    ///
    /// Es el camino SINCRONO: el sector de FAT que falte se lee aqui mismo, por
    /// la cache de un sector del volumen. `None` si no queda nada que leer o el
    /// cluster no es de este volumen.
    pub fn planear_tramo(&mut self, cluster: u32, ya: usize, file_size: u32, tope: usize) -> Option<Tramo> {
        // ** La cache se COPIA y se devuelve al final, y no se presta: el plan
        // mira el volumen (`&self`) mientras la lectura escribe en la cache, y
        // prestar las dos cosas a la vez no compila. Son 512 bytes.
        //
        // [!] Y no se vuelve a llamar al plan puro con una ventana nueva por
        // cada sector que falte: con UNA sola de cache, una cadena que cruza de
        // un sector de la FAT al siguiente pediria el primero, luego el
        // segundo, luego otra vez el primero... El plan sigue su vuelta y la
        // lectura ocurre DENTRO de ella.
        let dev = self.dev;
        let base = self.part_lba;
        let mut lba = self.fat_cache_lba;
        let mut sector = self.fat_cache;
        let plan = self.planear_con(cluster, ya, file_size, tope, |s, idx| {
            if !super::leer_en_cache(dev, base, s, &mut lba, &mut sector) {
                // Lo que no se pudo leer se lee como FIN DE CADENA: se entrega
                // lo que hay y se dice que se acabo. Un fichero corto se nota;
                // un bucle no.
                return Some(0);
            }
            let b = &sector[idx..idx + 4];
            Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        });
        self.fat_cache_lba = lba;
        self.fat_cache = sector;
        match plan {
            Plan::Tramo(t) => Some(t),
            // Con la lectura dentro no puede faltar nada; si faltara, no hay
            // tramo que dar y decir "nada" es lo que no inventa.
            Plan::Nada | Plan::Falta(_) => None,
        }
    }

    /// **Que traer para cubrir el sector `falta` de la FAT.** Devuelve
    /// `(primer sector RELATIVO, primer sector ABSOLUTO, cuantos)`.
    ///
    /// Una ventana de `max` sectores alineada a `max` dentro de la FAT --una
    /// cadena se recorre hacia delante, y alineada tampoco se sale por el
    /// principio-- y recortada donde acaba la primera copia de la FAT. `None`
    /// si `falta` no es un sector de la FAT: pedirlo seria leer cualquier cosa
    /// como si fueran entradas.
    pub fn ventana_fat(&self, falta: u64, max: u64) -> Option<(u64, u64, u16)> {
        let inicio = self.fat_start as u64;
        let fin = inicio + self.fat_size_sectors as u64;
        if max == 0 || falta < inicio || falta >= fin {
            return None;
        }
        let desde = inicio + (falta - inicio) / max * max;
        let n = (fin - desde).min(max).min(SECTORES_MAX as u64);
        Some((desde, self.abs(desde), n as u16))
    }

    /// **La cadena, recorrida una sola vez**, con las entradas de donde diga
    /// `entrada(sector, idx)`: la cruda, o `None` si no la tiene -- y entonces
    /// el plan devuelve `Falta(sector)` sin tocar nada.
    fn planear_con(
        &self,
        cluster: u32,
        ya: usize,
        file_size: u32,
        tope: usize,
        mut entrada: impl FnMut(u64, usize) -> Option<u32>,
    ) -> Plan {
        let fin = file_size as usize;
        if ya >= fin {
            return Plan::Nada;
        }
        let Some(lba) = self.lba_valido(cluster) else { return Plan::Nada };
        let spc = self.sectors_per_cluster as usize;
        let del_cluster = spc * 512;
        let tope = tope.max(del_cluster);
        let mut ultimo = cluster;
        let mut n = 1usize;
        let mut siguiente = 0u32;
        loop {
            let cubierto = n * del_cluster;
            if ya + cubierto >= fin {
                // El fichero acaba dentro de este tramo: no hay por donde seguir.
                break;
            }
            let (sector, idx) = self.donde_en_la_fat(ultimo);
            let Some(crudo) = entrada(sector, idx) else { return Plan::Falta(sector) };
            let otro = match self.siguiente_de(crudo) {
                Some(c) => c,
                // Fin de cadena antes que fin de fichero: se entrega lo que hay
                // y se dice que se acabo. Un fichero corto se nota; un bucle no.
                None => break,
            };
            let cabe = cubierto < tope && (n + 1) * spc <= SECTORES_MAX;
            if otro == ultimo + 1 && cabe {
                ultimo = otro;
                n += 1;
            } else {
                siguiente = otro;
                break;
            }
        }
        let bytes = (n * del_cluster).min(fin - ya);
        Plan::Tramo(Tramo {
            // Por `abs`, como todo camino directo: una suma escrita a mano aqui
            // seria la tercera copia de la que ya fallo una vez.
            lba: self.abs(lba),
            sectores: bytes.div_ceil(512) as u16,
            bytes,
            siguiente,
        })
    }
}

