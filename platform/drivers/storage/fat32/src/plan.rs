//! **PLANEAR UN TRAMO: donde esta en el disco lo que toca leer, SIN leerlo.**
//! (paso D1 del plan del disco, 2026-09-23)
//!
//! ## Por que existe
//!
//! `leer_tramo` sigue la cadena Y mueve los datos, cluster a cluster, en la
//! misma llamada. Para el HILO DEL DISCO eso no sirve: el hilo tiene que poder
//! mandar UNA orden de datos, soltar el disco y DORMIR hasta que el aparato
//! acabe. Asi que aqui se separa lo que dura poco --seguir la cadena, que son
//! lecturas de la FAT que casi siempre salen de su cache-- de lo que dura mucho
//! --mover los bytes--, y lo segundo lo hace quien llama.
//!
//! ## Lo que devuelve, y el contrato
//!
//! Un [`Tramo`]: los clusters SEGUIDOS en el disco a partir del cursor, juntos
//! en una sola orden. Un fichero escrito de una vez suele estar entero seguido,
//! y entonces 128 KiB son UNA orden en vez de una por cluster.
//!
//! [!] **El LBA es ABSOLUTO** (ya lleva `part_lba`): lo consume el kernel para
//! programar el AHCI directamente. Es la suma que costo dos tandas de fotos el
//! 10-08 (ver `leer_de_una_particion_que_no_empieza_en_cero`), y por eso tiene
//! su propia prueba con el volumen desplazado.
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

impl FatVolume {
    /// **El siguiente tramo contiguo** a partir de `cluster`, que contiene el
    /// byte `ya` del fichero (frontera de cluster, el mismo contrato que
    /// `leer_tramo`). Junta clusters mientras sean consecutivos en el disco, sin
    /// pasar de `tope` bytes ni de [`SECTORES_MAX`].
    ///
    /// `None` si no queda nada que leer o el cluster no es de este volumen.
    pub fn planear_tramo(&mut self, cluster: u32, ya: usize, file_size: u32, tope: usize) -> Option<Tramo> {
        let fin = file_size as usize;
        if ya >= fin {
            return None;
        }
        let lba = self.lba_valido(cluster)?;
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
            let otro = match self.read_fat_entry(ultimo) {
                Some(c) if self.cluster_valido(c) => c,
                // Fin de cadena antes que fin de fichero: se entrega lo que hay
                // y se dice que se acabo. Un fichero corto se nota; un bucle no.
                _ => break,
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
        Some(Tramo {
            // Por `abs`, como todo camino directo: una suma escrita a mano aqui
            // seria la tercera copia de la que ya fallo una vez.
            lba: self.abs(lba),
            sectores: bytes.div_ceil(512) as u16,
            bytes,
            siguiente,
        })
    }
}
