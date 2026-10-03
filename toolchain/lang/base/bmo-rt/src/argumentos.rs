//! **`argc` y `argv` desde `TASK_OP_ARGUMENTOS`.**
//!
//! En BMO-X no hay una pila con `argc, argv, envp, auxv` puesta por el
//! cargador: el kernel guarda el texto que iba detras de la ruta al lanzar
//! (`run doom.bex -iwad doom1.wad`, hasta 128 bytes) y el proceso lo pide de
//! 8 en 8. `crt0` lo trae a una linea fija y aqui se parte.
//!
//! ```text
//!    argv[0]     "" -- el programa no se nombra a si mismo: el kernel sabe
//!                de donde salio (OP_MI_PAQUETE), y lo da por handle, no
//!                por nombre
//!    argv[1..]   las palabras, separadas por espacios; "entre comillas" una
//!                con espacios dentro (un nombre largo de ESTRATOS)
//!    argv[argc]  nulo, como C promete
//! ```
//!
//! No hay entorno (`envp`): BMO-X no tiene variables de entorno.

/// Lo que el kernel deja de argumentos, mas el cero.
pub const LINEA: usize = 128 + 1;

/// Palabras como mucho (sin contar `argv[0]`): 128 bytes dan para 64.
pub const MAX_ARGS: usize = 64;

/// Partir `linea[..n]` en `argv`, poniendo un cero detras de cada palabra.
/// `cero` es una cadena vacia para `argv[0]`. Devuelve `argc`.
pub fn partir(linea: &mut [u8], n: usize, argv: &mut [*const u8], cero: *const u8) -> usize {
    let n = n.min(linea.len().saturating_sub(1));
    let tope = argv.len().saturating_sub(1);
    if tope == 0 {
        return 0;
    }
    argv[0] = cero;
    let mut argc = 1usize;
    let mut i = 0usize;
    while i < n && argc < tope {
        while i < n && linea[i] == b' ' {
            i += 1;
        }
        if i >= n {
            break;
        }
        let comillas = linea[i] == b'"';
        if comillas {
            i += 1;
        }
        let desde = i;
        while i < n && if comillas { linea[i] != b'"' } else { linea[i] != b' ' } {
            i += 1;
        }
        // Cerrar la palabra: su cero pisa el espacio o la comilla de detras
        // (la linea lleva uno de sobra al final).
        linea[i] = 0;
        argv[argc] = linea[desde..].as_ptr();
        argc += 1;
        i += 1;
    }
    argv[argc] = core::ptr::null();
    argc
}

#[cfg(test)]
mod tests {
    use super::*;

    fn palabras(texto: &str) -> std::vec::Vec<std::string::String> {
        let mut linea = [0u8; LINEA];
        linea[..texto.len()].copy_from_slice(texto.as_bytes());
        let mut argv = [core::ptr::null(); MAX_ARGS + 2];
        let cero = [0u8];
        let argc = partir(&mut linea, texto.len(), &mut argv, cero.as_ptr());
        assert!(argv[argc].is_null(), "argv[argc] es nulo");
        (0..argc)
            .map(|k| unsafe {
                let p = argv[k];
                let n = (0..).position(|j| *p.add(j) == 0).unwrap();
                std::string::String::from_utf8_lossy(core::slice::from_raw_parts(p, n)).into_owned()
            })
            .collect()
    }

    #[test]
    fn sin_argumentos() {
        assert_eq!(palabras(""), [""]);
        assert_eq!(palabras("   "), [""]);
    }

    #[test]
    fn palabras_y_espacios() {
        assert_eq!(palabras("-iwad doom1.wad"), ["", "-iwad", "doom1.wad"]);
        assert_eq!(palabras("  a   b  "), ["", "a", "b"]);
    }

    #[test]
    fn comillas() {
        assert_eq!(palabras("-file \"Mi Mapa.wad\" -x"), ["", "-file", "Mi Mapa.wad", "-x"]);
        assert_eq!(palabras("\"sin cerrar"), ["", "sin cerrar"]);
    }

    #[test]
    fn la_linea_entera_cabe() {
        let t: std::string::String = (0..64).map(|_| "a ").collect();
        let v = palabras(&t[..128]);
        assert_eq!(v.len(), 1 + 64);
    }
}
