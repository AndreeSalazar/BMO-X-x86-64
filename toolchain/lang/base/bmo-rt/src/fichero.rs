//! **Los ficheros de C sobre `KIND_ARCHIVO`**: `fopen`, `fread`, `fwrite`,
//! `fseek`, `fgets`, `fclose`... y `stdout`/`stderr` a la consola.
//!
//! Era el punto 1 de lo que le faltaba a esta libc: sin `fopen`, DOOM no carga
//! su WAD. Aqui no hay descriptores ni `open(2)`: un `FILE` es un HANDLE que el
//! kernel concedio, y lo de alrededor es lo que BMO-X da.
//!
//! ```text
//!    "r" "rb"    TASK_OP_ARCHIVO_ABRIR   leer; el cursor salta (fseek)
//!    "w" "wb"    TASK_OP_ARCHIVO_CREAR   escribir; NADA llega al disco hasta
//!                                        fclose (o exit, que los cierra)
//!    la ruta     por el renglon TASK_OP_RUTA, de 8 en 8: `datos/x.txt`, nombres
//!                8.3 en FAT32, la carpeta tiene que existir
//!    a la vez    16 abiertos, los mismos que deja el kernel
//!    leer        ARCH_OP_LEER_EN: el kernel copia DIRECTO a un bloque de
//!                KIND_MEMORIA. Si el destino es del monton (un `malloc`), es
//!                una llamada sin copia; si no, pasa por el bufer del FILE,
//!                que tambien es del monton
//!    escribir    ARCH_OP_ESCRIBIR_DE, el espejo, desde el bufer
//!    stdin       no hay: la entrada de BMO-X es KIND_INPUT, no un fichero.
//!                Leerlo da fin de fichero
//! ```
//!
//! [!] Lo que NO hay, y por que: `"a"` y `"r+"`/`"w+"` (el kernel no abre un
//! fichero para leer y escribir a la vez, ni agrega a uno que existe) y saltar
//! en uno de escritura (el kernel acumula en orden). Se contestan con nulo o
//! con -1, no con algo parecido.
//!
//! La logica va sobre el rasgo [`Disco`] para probarla en el anfitrion con un
//! disco de mentira; el de verdad es [`Kernel`].

use crate::syscall;
use bmo_abi::syscalls::surface::{
    ARCH_OP_CERRAR, ARCH_OP_ESCRIBIR, ARCH_OP_ESCRIBIR_DE, ARCH_OP_LEER, ARCH_OP_LEER_EN, ARCH_OP_MEDIDA,
    ARCH_OP_SALTAR,
};
use core::ptr;

/// Lo que mide el bufer de cada `FILE`.
pub const BUFER: usize = 16 * 1024;

/// Los abiertos a la vez (los del kernel), mas `stdin`, `stdout` y `stderr`.
pub const FICHEROS: usize = 3 + 16;

/// `fgetc` y compania al acabarse.
pub const EOF: i32 = -1;

/// Lo que un `FILE` necesita de abajo.
pub trait Disco {
    /// Abrir para leer o para escribir: el handle.
    fn abrir(&self, ruta: &[u8], escribir: bool) -> Option<u64>;
    /// Bytes que quedan desde el cursor del kernel (leer).
    fn medida(&self, h: u64) -> u64;
    /// Leer desde el cursor del kernel a `dst`. Los traidos; 0 = se acabo.
    fn leer(&self, h: u64, dst: *mut u8, n: usize) -> usize;
    /// Escribir `src` detras de lo escrito. Los aceptados.
    fn escribir(&self, h: u64, src: *const u8, n: usize) -> usize;
    /// Poner el cursor del kernel en `pos`. Donde quedo.
    fn saltar(&self, h: u64, pos: u64) -> u64;
    /// Cerrar. En uno de escritura, es cuando llega al disco.
    fn cerrar(&self, h: u64) -> bool;
    /// La consola (stdout y stderr).
    fn consola(&self, b: &[u8]);
    /// Si `[p, p + n)` se puede leer o escribir de una llamada (es memoria
    /// que el kernel concedio). Si no, se pasa por el bufer.
    fn directo(&self, p: *const u8, n: usize) -> bool;
    /// Un bufer de [`BUFER`] bytes, o nulo.
    fn bufer(&self) -> *mut u8;
    fn soltar_bufer(&self, p: *mut u8);
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Clase {
    Libre,
    Entrada,
    Consola,
    Lee,
    Escribe,
}

/// **Un `FILE`.** Para C es opaco: solo se pasa su puntero.
#[repr(C)]
pub struct Fichero {
    clase: Clase,
    handle: u64,
    /// Leer: donde esta el cursor del kernel. Escribir: lo que ya bajo.
    kpos: u64,
    /// Leer: lo que media el fichero al abrirlo.
    total: u64,
    buf: *mut u8,
    /// Leer: lo que queda en el bufer es `[ini, fin)`. Escribir: `[0, fin)`.
    ini: usize,
    fin: usize,
    eof: bool,
    error: bool,
}

const LIBRE: Fichero = Fichero {
    clase: Clase::Libre,
    handle: 0,
    kpos: 0,
    total: 0,
    buf: ptr::null_mut(),
    ini: 0,
    fin: 0,
    eof: false,
    error: false,
};

impl Fichero {
    const fn de(clase: Clase) -> Self {
        Self { clase, ..LIBRE }
    }

    fn pos(&self) -> u64 {
        match self.clase {
            Clase::Lee => self.kpos - (self.fin - self.ini) as u64,
            Clase::Escribe => self.kpos + self.fin as u64,
            _ => 0,
        }
    }

    /// Bajar lo que hay en el bufer de escritura.
    fn vaciar(&mut self, d: &impl Disco) -> bool {
        if self.clase != Clase::Escribe || self.fin == 0 {
            return true;
        }
        let n = d.escribir(self.handle, self.buf, self.fin);
        self.kpos += n as u64;
        let bien = n == self.fin;
        self.fin = 0;
        if !bien {
            self.error = true;
        }
        bien
    }

    /// Llenar el bufer de lectura. `false` si no llego nada.
    fn llenar(&mut self, d: &impl Disco) -> bool {
        self.ini = 0;
        self.fin = 0;
        let n = d.leer(self.handle, self.buf, BUFER);
        self.kpos += n as u64;
        self.fin = n;
        if n == 0 {
            self.eof = true;
        }
        n > 0
    }

    /// `fread` en bytes: los que se pusieron en `dst`.
    pub fn leer(&mut self, d: &impl Disco, dst: *mut u8, n: usize) -> usize {
        if self.clase != Clase::Lee {
            if self.clase == Clase::Entrada {
                self.eof = true;
            }
            return 0;
        }
        let mut hecho = 0usize;
        while hecho < n {
            let hay = self.fin - self.ini;
            if hay > 0 {
                let k = hay.min(n - hecho);
                unsafe { ptr::copy_nonoverlapping(self.buf.add(self.ini), dst.add(hecho), k) };
                self.ini += k;
                hecho += k;
                continue;
            }
            let falta = n - hecho;
            let p = unsafe { dst.add(hecho) };
            if falta >= BUFER && d.directo(p, falta) {
                // De una: el kernel lo deja donde va.
                let k = d.leer(self.handle, p, falta);
                self.kpos += k as u64;
                hecho += k;
                if k == 0 {
                    self.eof = true;
                    break;
                }
                continue;
            }
            if !self.llenar(d) {
                break;
            }
        }
        hecho
    }

    /// `fwrite` en bytes: los aceptados.
    pub fn escribir(&mut self, d: &impl Disco, src: *const u8, n: usize) -> usize {
        match self.clase {
            Clase::Consola => {
                d.consola(unsafe { core::slice::from_raw_parts(src, n) });
                n
            }
            Clase::Escribe => {
                let mut hecho = 0usize;
                while hecho < n {
                    let falta = n - hecho;
                    let p = unsafe { src.add(hecho) };
                    if self.fin == 0 && falta >= BUFER && d.directo(p, falta) {
                        let k = d.escribir(self.handle, p, falta);
                        self.kpos += k as u64;
                        hecho += k;
                        if k < falta {
                            self.error = true;
                            break;
                        }
                        continue;
                    }
                    let k = (BUFER - self.fin).min(falta);
                    unsafe { ptr::copy_nonoverlapping(p, self.buf.add(self.fin), k) };
                    self.fin += k;
                    hecho += k;
                    if self.fin == BUFER && !self.vaciar(d) {
                        break;
                    }
                }
                hecho
            }
            _ => {
                self.error = true;
                0
            }
        }
    }

    /// `fgetc`.
    pub fn byte(&mut self, d: &impl Disco) -> i32 {
        let mut b = 0u8;
        if self.leer(d, &mut b, 1) == 1 {
            b as i32
        } else {
            EOF
        }
    }

    /// `ungetc`: solo el byte que se acaba de leer (lo que C promete).
    pub fn devolver(&mut self, c: i32) -> i32 {
        if c == EOF || self.clase != Clase::Lee || self.ini == 0 {
            return EOF;
        }
        self.ini -= 1;
        unsafe { *self.buf.add(self.ini) = c as u8 };
        self.eof = false;
        c
    }

    /// `fgets`: hasta el salto (incluido) o `n - 1` bytes, con su cero.
    pub fn linea(&mut self, d: &impl Disco, s: *mut u8, n: usize) -> bool {
        if n == 0 {
            return false;
        }
        let mut k = 0usize;
        while k + 1 < n {
            let c = self.byte(d);
            if c == EOF {
                break;
            }
            unsafe { *s.add(k) = c as u8 };
            k += 1;
            if c == b'\n' as i32 {
                break;
            }
        }
        unsafe { *s.add(k) = 0 };
        k > 0
    }

    /// `fseek`: 0 si se pudo, -1 si no.
    pub fn saltar(&mut self, d: &impl Disco, desp: i64, desde: i32) -> i32 {
        let base = match desde {
            0 => 0i64,
            1 => self.pos() as i64,
            2 if self.clase == Clase::Lee => self.total as i64,
            _ => return -1,
        };
        let Some(destino) = base.checked_add(desp).filter(|&v| v >= 0) else { return -1 };
        match self.clase {
            Clase::Lee => {
                self.ini = 0;
                self.fin = 0;
                self.kpos = d.saltar(self.handle, destino as u64);
                self.eof = false;
                if self.kpos == destino as u64 {
                    0
                } else {
                    -1
                }
            }
            // Escribir: solo quedarse donde esta.
            Clase::Escribe if destino as u64 == self.pos() => 0,
            _ => -1,
        }
    }

    /// `ftell`.
    pub fn donde(&self) -> i64 {
        match self.clase {
            Clase::Lee | Clase::Escribe => self.pos() as i64,
            _ => -1,
        }
    }

    /// `feof`.
    pub fn al_final(&self) -> bool {
        self.eof
    }

    /// `ferror`.
    pub fn con_error(&self) -> bool {
        self.error
    }

    /// `clearerr`.
    pub fn limpiar(&mut self) {
        self.eof = false;
        self.error = false;
    }

    /// `fflush`.
    pub fn bajar(&mut self, d: &impl Disco) -> bool {
        self.vaciar(d)
    }

    /// `fclose`: baja lo que quede y cierra. `true` si todo llego.
    fn cerrar(&mut self, d: &impl Disco) -> bool {
        let mut bien = true;
        if matches!(self.clase, Clase::Lee | Clase::Escribe) {
            bien &= self.vaciar(d);
            bien &= d.cerrar(self.handle);
            if !self.buf.is_null() {
                d.soltar_bufer(self.buf);
            }
            *self = LIBRE;
        }
        bien
    }
}

/// **La tabla de `FILE`**: 0 `stdin`, 1 `stdout`, 2 `stderr`, y 16 mas.
pub struct Tabla {
    pub f: [Fichero; FICHEROS],
}

impl Tabla {
    pub const fn new() -> Self {
        let mut f = [LIBRE; FICHEROS];
        f[0] = Fichero::de(Clase::Entrada);
        f[1] = Fichero::de(Clase::Consola);
        f[2] = Fichero::de(Clase::Consola);
        Self { f }
    }

    /// `fopen(ruta, modo)`: el `FILE`, o nulo.
    pub fn abrir(&mut self, d: &impl Disco, ruta: &[u8], modo: &[u8]) -> *mut Fichero {
        let escribe = match modo {
            b"r" | b"rb" | b"rt" => false,
            b"w" | b"wb" | b"wt" => true,
            _ => return ptr::null_mut(),
        };
        let Some(i) = (3..FICHEROS).find(|&i| self.f[i].clase == Clase::Libre) else { return ptr::null_mut() };
        let buf = d.bufer();
        if buf.is_null() {
            return ptr::null_mut();
        }
        let Some(h) = d.abrir(ruta, escribe) else {
            d.soltar_bufer(buf);
            return ptr::null_mut();
        };
        let total = if escribe { 0 } else { d.medida(h) };
        self.f[i] = Fichero {
            clase: if escribe { Clase::Escribe } else { Clase::Lee },
            handle: h,
            total,
            buf,
            ..LIBRE
        };
        &mut self.f[i]
    }

    /// `fclose`. `stdin`/`stdout`/`stderr` no se cierran (no hay nada debajo).
    pub fn cerrar(&mut self, d: &impl Disco, f: *mut Fichero) -> bool {
        match self.indice(f) {
            Some(i) if i >= 3 => self.f[i].cerrar(d),
            Some(_) => true,
            None => false,
        }
    }

    /// Al salir: cerrar todo, que es cuando lo escrito llega al disco.
    pub fn cerrar_todos(&mut self, d: &impl Disco) {
        for i in 3..FICHEROS {
            self.f[i].cerrar(d);
        }
    }

    /// Un `FILE*` de C a la entrada de la tabla, si es uno de ella.
    pub fn indice(&self, f: *const Fichero) -> Option<usize> {
        let base = self.f.as_ptr() as usize;
        let p = f as usize;
        let t = core::mem::size_of::<Fichero>();
        (p >= base && p < base + FICHEROS * t && (p - base) % t == 0).then(|| (p - base) / t)
    }

    pub fn de(&mut self, f: *mut Fichero) -> Option<&mut Fichero> {
        self.indice(f).map(move |i| &mut self.f[i]).filter(|x| x.clase != Clase::Libre)
    }
}

impl Default for Tabla {
    fn default() -> Self {
        Self::new()
    }
}

// == El disco de verdad ======================================================

/// El kernel de BMO-X.
pub struct Kernel;

impl Disco for Kernel {
    fn abrir(&self, ruta: &[u8], escribir: bool) -> Option<u64> {
        unsafe { syscall::archivo_abrir(ruta, escribir) }
    }

    fn medida(&self, h: u64) -> u64 {
        unsafe { syscall::archivo_op(h, ARCH_OP_MEDIDA, 0, 0, 0) }
    }

    fn leer(&self, h: u64, dst: *mut u8, n: usize) -> usize {
        if let Some((bloque, desde)) = crate::heap::bloque_de(dst, n) {
            // Una llamada, sin copia: el destino es un bloque que el kernel dio.
            let mut hecho = 0usize;
            while hecho < n {
                let k = unsafe {
                    syscall::archivo_op(h, ARCH_OP_LEER_EN, bloque, desde + hecho as u64, (n - hecho) as u64)
                } as usize;
                if k == 0 {
                    break;
                }
                hecho += k.min(n - hecho);
            }
            return hecho;
        }
        // Siete bytes por llamada: solo para destinos que no son del monton.
        let mut hecho = 0usize;
        while hecho < n {
            let v = unsafe { syscall::archivo_op(h, ARCH_OP_LEER, 0, 0, 0) };
            let k = ((v >> 56) as usize).min(7);
            if k == 0 {
                break;
            }
            for (j, b) in v.to_le_bytes()[..k].iter().enumerate() {
                if hecho + j < n {
                    unsafe { *dst.add(hecho + j) = *b };
                }
            }
            hecho += k.min(n - hecho);
        }
        hecho
    }

    fn escribir(&self, h: u64, src: *const u8, n: usize) -> usize {
        if let Some((bloque, desde)) = crate::heap::bloque_de(src, n) {
            return unsafe { syscall::archivo_op(h, ARCH_OP_ESCRIBIR_DE, bloque, desde, n as u64) } as usize;
        }
        let mut hecho = 0usize;
        while hecho < n {
            let k = (n - hecho).min(7);
            let mut w = [0u8; 8];
            unsafe { ptr::copy_nonoverlapping(src.add(hecho), w.as_mut_ptr(), k) };
            w[7] = k as u8;
            let puestos = unsafe { syscall::archivo_op(h, ARCH_OP_ESCRIBIR, u64::from_le_bytes(w), 0, 0) } as usize;
            hecho += puestos.min(k);
            if puestos < k {
                break;
            }
        }
        hecho
    }

    fn saltar(&self, h: u64, pos: u64) -> u64 {
        unsafe { syscall::archivo_op(h, ARCH_OP_SALTAR, pos, 0, 0) }
    }

    fn cerrar(&self, h: u64) -> bool {
        let r = unsafe { bmo_abi::syscalls::surface::invoke(h, ARCH_OP_CERRAR, 0, 0, 0, 0) };
        r.is_ok()
    }

    fn consola(&self, b: &[u8]) {
        unsafe { syscall::consola_escribir(b.as_ptr(), b.len() as u64) }
    }

    fn directo(&self, p: *const u8, n: usize) -> bool {
        crate::heap::bloque_de(p, n).is_some()
    }

    fn bufer(&self) -> *mut u8 {
        crate::heap::malloc(BUFER)
    }

    fn soltar_bufer(&self, p: *mut u8) {
        crate::heap::free(p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::cell::RefCell;
    use std::vec::Vec;

    /// Un disco en memoria: ficheros por nombre, y la cuenta de llamadas.
    #[derive(Default)]
    struct Falso {
        ficheros: RefCell<Vec<(Vec<u8>, Vec<u8>)>>,
        abiertos: RefCell<Vec<(usize, u64, bool, Vec<u8>)>>, // (fichero, cursor, escribe, lo escrito)
        consola: RefCell<Vec<u8>>,
        lecturas: core::cell::Cell<u32>,
        directo: bool,
    }

    impl Falso {
        fn con(nombre: &str, datos: &[u8]) -> Self {
            let f = Self::default();
            f.ficheros.borrow_mut().push((nombre.as_bytes().to_vec(), datos.to_vec()));
            f
        }
        fn contenido(&self, nombre: &str) -> Option<Vec<u8>> {
            self.ficheros.borrow().iter().find(|(n, _)| n == nombre.as_bytes()).map(|(_, d)| d.clone())
        }
    }

    impl Disco for Falso {
        fn abrir(&self, ruta: &[u8], escribir: bool) -> Option<u64> {
            let mut fs = self.ficheros.borrow_mut();
            let i = match fs.iter().position(|(n, _)| n == ruta) {
                Some(i) => i,
                None if escribir => {
                    fs.push((ruta.to_vec(), Vec::new()));
                    fs.len() - 1
                }
                None => return None,
            };
            let mut a = self.abiertos.borrow_mut();
            a.push((i, 0, escribir, Vec::new()));
            Some(a.len() as u64 - 1)
        }
        fn medida(&self, h: u64) -> u64 {
            let a = &self.abiertos.borrow()[h as usize];
            (self.ficheros.borrow()[a.0].1.len() as u64).saturating_sub(a.1)
        }
        fn leer(&self, h: u64, dst: *mut u8, n: usize) -> usize {
            self.lecturas.set(self.lecturas.get() + 1);
            let mut a = self.abiertos.borrow_mut();
            let e = &mut a[h as usize];
            let fs = self.ficheros.borrow();
            let datos = &fs[e.0].1;
            let desde = (e.1 as usize).min(datos.len());
            let k = n.min(datos.len() - desde);
            unsafe { ptr::copy_nonoverlapping(datos[desde..].as_ptr(), dst, k) };
            e.1 += k as u64;
            k
        }
        fn escribir(&self, h: u64, src: *const u8, n: usize) -> usize {
            let mut a = self.abiertos.borrow_mut();
            a[h as usize].3.extend_from_slice(unsafe { core::slice::from_raw_parts(src, n) });
            n
        }
        fn saltar(&self, h: u64, pos: u64) -> u64 {
            let mut a = self.abiertos.borrow_mut();
            let largo = self.ficheros.borrow()[a[h as usize].0].1.len() as u64;
            a[h as usize].1 = pos.min(largo);
            a[h as usize].1
        }
        fn cerrar(&self, h: u64) -> bool {
            let a = self.abiertos.borrow();
            let e = &a[h as usize];
            if e.2 {
                self.ficheros.borrow_mut()[e.0].1 = e.3.clone();
            }
            true
        }
        fn consola(&self, b: &[u8]) {
            self.consola.borrow_mut().extend_from_slice(b);
        }
        fn directo(&self, _p: *const u8, _n: usize) -> bool {
            self.directo
        }
        fn bufer(&self) -> *mut u8 {
            std::boxed::Box::into_raw(std::vec![0u8; BUFER].into_boxed_slice()) as *mut u8
        }
        fn soltar_bufer(&self, p: *mut u8) {
            unsafe { drop(std::boxed::Box::from_raw(core::ptr::slice_from_raw_parts_mut(p, BUFER))) }
        }
    }

    fn datos(n: usize) -> Vec<u8> {
        (0..n).map(|i| (i * 7 + i / 251) as u8).collect()
    }

    #[test]
    fn leer_entero_y_por_trozos() {
        let todo = datos(50_000);
        let d = Falso::con("doom1.wad", &todo);
        let mut t = Tabla::new();
        let f = t.abrir(&d, b"doom1.wad", b"rb");
        assert!(!f.is_null());
        let f = t.de(f).unwrap();
        let mut a = std::vec![0u8; 12];
        assert_eq!(f.leer(&d, a.as_mut_ptr(), 12), 12);
        assert_eq!(&a[..], &todo[..12]);
        let mut b = std::vec![0u8; 50_000];
        assert_eq!(f.leer(&d, b.as_mut_ptr(), 50_000), 50_000 - 12);
        assert_eq!(&b[..50_000 - 12], &todo[12..]);
        assert!(f.al_final());
        assert_eq!(f.donde(), 50_000);
    }

    /// El directorio de lumps de un WAD esta AL FINAL: fseek desde el final,
    /// leer, y volver a donde se estaba.
    #[test]
    fn saltar_como_un_wad() {
        let todo = datos(40_000);
        let d = Falso::con("doom1.wad", &todo);
        let mut t = Tabla::new();
        let p = t.abrir(&d, b"doom1.wad", b"r");
        let f = t.de(p).unwrap();
        assert_eq!(f.saltar(&d, -16, 2), 0);
        assert_eq!(f.donde(), 40_000 - 16);
        let mut x = [0u8; 16];
        assert_eq!(f.leer(&d, x.as_mut_ptr(), 16), 16);
        assert_eq!(&x[..], &todo[40_000 - 16..]);
        assert_eq!(f.saltar(&d, 100, 0), 0);
        assert_eq!(f.byte(&d), todo[100] as i32);
        assert_eq!(f.saltar(&d, 10, 1), 0);
        assert_eq!(f.donde(), 111);
        assert_eq!(f.byte(&d), todo[111] as i32);
        assert_eq!(f.saltar(&d, -1, 0), -1);
    }

    /// Con destino directo (memoria del monton), lo grande va de UNA llamada.
    #[test]
    fn lo_grande_va_directo() {
        let todo = datos(BUFER * 4);
        let mut d = Falso::con("e1m1.lmp", &todo);
        d.directo = true;
        let mut t = Tabla::new();
        let p = t.abrir(&d, b"e1m1.lmp", b"rb");
        let f = t.de(p).unwrap();
        let mut b = std::vec![0u8; BUFER * 4];
        assert_eq!(f.leer(&d, b.as_mut_ptr(), BUFER * 4), BUFER * 4);
        assert_eq!(b, todo);
        assert_eq!(d.lecturas.get(), 1);
    }

    #[test]
    fn lineas_y_devolver() {
        let d = Falso::con("cfg.txt", b"uno\ndos dos\n\ntres");
        let mut t = Tabla::new();
        let p = t.abrir(&d, b"cfg.txt", b"r");
        let f = t.de(p).unwrap();
        let mut l = [0u8; 64];
        let leer = |f: &mut Fichero, l: &mut [u8; 64]| {
            let ok = f.linea(&d, l.as_mut_ptr(), 64);
            let n = l.iter().position(|&c| c == 0).unwrap();
            (ok, std::string::String::from_utf8_lossy(&l[..n]).into_owned())
        };
        assert_eq!(leer(f, &mut l), (true, "uno\n".into()));
        let c = f.byte(&d);
        assert_eq!(c, b'd' as i32);
        assert_eq!(f.devolver(c), c);
        assert_eq!(leer(f, &mut l), (true, "dos dos\n".into()));
        assert_eq!(leer(f, &mut l), (true, "\n".into()));
        assert_eq!(leer(f, &mut l), (true, "tres".into()));
        assert_eq!(leer(f, &mut l), (false, "".into()));
        assert!(f.al_final());
        // Corta a n - 1 con su cero.
        let p = t.abrir(&d, b"cfg.txt", b"r");
        let f = t.de(p).unwrap();
        let mut c4 = [9u8; 4];
        assert!(f.linea(&d, c4.as_mut_ptr(), 4));
        assert_eq!(&c4, b"uno\0");
    }

    /// Escribir no llega al disco hasta cerrar, y llega entero.
    #[test]
    fn escribir_llega_al_cerrar() {
        let d = Falso::default();
        let mut t = Tabla::new();
        let p = t.abrir(&d, b"datos/save.dat", b"wb");
        assert!(!p.is_null());
        let grande = datos(BUFER * 2 + 123);
        {
            let f = t.de(p).unwrap();
            assert_eq!(f.escribir(&d, b"BMO".as_ptr(), 3), 3);
            assert_eq!(f.escribir(&d, grande.as_ptr(), grande.len()), grande.len());
            assert_eq!(f.donde(), 3 + grande.len() as i64);
            assert_eq!(f.saltar(&d, 0, 1), 0, "quedarse donde esta si");
            assert_eq!(f.saltar(&d, 0, 0), -1, "volver atras no");
        }
        assert_eq!(d.contenido("datos/save.dat"), Some(Vec::new()));
        assert!(t.cerrar(&d, p));
        let mut esperado = b"BMO".to_vec();
        esperado.extend_from_slice(&grande);
        assert_eq!(d.contenido("datos/save.dat"), Some(esperado));
    }

    #[test]
    fn modos_y_limites() {
        let d = Falso::con("a.txt", b"x");
        let mut t = Tabla::new();
        for m in [&b"a"[..], b"r+", b"w+", b"", b"x"] {
            assert!(t.abrir(&d, b"a.txt", m).is_null(), "{m:?}");
        }
        assert!(t.abrir(&d, b"no.txt", b"r").is_null());
        // 16 a la vez, como el kernel.
        let mut v = Vec::new();
        for _ in 0..16 {
            let p = t.abrir(&d, b"a.txt", b"r");
            assert!(!p.is_null());
            v.push(p);
        }
        assert!(t.abrir(&d, b"a.txt", b"r").is_null());
        assert!(t.cerrar(&d, v[3]));
        assert!(!t.abrir(&d, b"a.txt", b"r").is_null());
        // Un puntero que no es de la tabla no es un FILE.
        let mut otro = LIBRE;
        assert!(!t.cerrar(&d, &mut otro));
    }

    #[test]
    fn consola_y_entrada() {
        let d = Falso::default();
        let mut t = Tabla::new();
        let out: *mut Fichero = &mut t.f[1];
        assert_eq!(t.de(out).unwrap().escribir(&d, b"hola\n".as_ptr(), 5), 5);
        assert_eq!(&d.consola.borrow()[..], b"hola\n");
        let inp: *mut Fichero = &mut t.f[0];
        let f = t.de(inp).unwrap();
        assert_eq!(f.byte(&d), EOF);
        assert!(f.al_final());
        assert!(t.cerrar(&d, out), "stdout no se cierra, pero no es un fallo");
    }

    /// `exit` cierra todo: lo escrito llega aunque el programa no cerrara.
    #[test]
    fn al_salir_se_cierra_todo() {
        let d = Falso::default();
        let mut t = Tabla::new();
        let p = t.abrir(&d, b"log.txt", b"w");
        t.de(p).unwrap().escribir(&d, b"fin".as_ptr(), 3);
        t.cerrar_todos(&d);
        assert_eq!(d.contenido("log.txt"), Some(b"fin".to_vec()));
        assert!(t.de(p).is_none());
    }
}
