//! **LA SOLAPA `historial`**: la cadena de versiones, dibujada.
//!
//! [consumo] NADA      no corre en reposo por su cuenta: pinta cuando el
//!                     compositor se lo pide, y el compositor solo pinta si
//!                     algo cambio (L6h)
//!
//! === Que muestra, y por que no se podia antes ===
//!
//! Cada estrato guarda un puntero a su padre, asi que la historia estaba en el
//! disco desde el primer dia. Lo que faltaba para poder pintarla no era el
//! recorrido -- eran las **dos columnas**:
//!
//! ```text
//!   la FECHA    el campo `tiempo` llevaba un CERO desde siempre.  19-08
//!   el NOMBRE   se escribia en todas, asi que no distinguia nada. 19-08
//! ```
//!
//! Sin esas dos, esta solapa habria sido una columna de filas identicas: un
//! grafo bien dibujado que no dice nada. Por eso se hizo despues y no antes.
//!
//! === Por que un grafo y no una lista ===
//!
//! Porque una version **no es un renglon de un registro: es un nodo con un
//! padre**. La cadena es la forma del dato, y dibujarla como lista obliga a
//! imaginarse las flechas. Es el mismo argumento que ya tiene el panel de nodos
//! del explorador, aplicado al otro eje: alli se baja por el arbol, aqui se va
//! hacia atras en el tiempo.
//!
//! === ** LO QUE SE DISTINGUE DE UN VISTAZO ===
//!
//! ```text
//!   la de ahora        en azul, como todo lo seleccionable del sistema
//!   con NOMBRE         lleva su nombre y un filo: no se suelta JAMAS
//!   automatica         sin nombre y apagada: el recolector puede llevarsela
//! ```
//!
//! Esa diferencia no es decorativa. **Un nombre es lo que hace permanente a una
//! version** (`Estrato::con_nombre`, section 9), asi que esta columna dice
//! literalmente cuales sobreviven a una limpieza y cuales no.
//!
//! === Y lo que NO hace todavia, dicho ===
//!
//! Mirar. No hay volver, ni revertir, ni suprimir. Volver a una version es
//! cambiar un puntero --en copy-on-write cuesta lo mismo con 4 KB que con 4 GB--
//! pero es una operacion que ESCRIBE, y esta solapa no escribe nada.
//!
//! Se dice aqui en vez de poner un boton que no hace nada: en un almacen, una
//! promesa que no ocurre es como se pierde el trabajo de alguien.

use bmo_userland as bmo;

use super::zonas::Zona;
use super::{INK, INK_DIM, INK_OK};

/// Alto de una tarjeta de version, y el hueco hasta la siguiente.
const CAJA_H: u32 = 50;
const HUECO: u32 = 8;
const ANCHO_MAX: u32 = 1100;
/// El alto del encabezado (el resumen de la cadena).
const CABEZA_H: u32 = 64;
/// La x del rail de la linea de tiempo, desde el borde de la zona.
const RAIL: u32 = 26;

/// ** LA LINEA DE TIEMPO, AL ESTILO DEL INICIO (01-10). El propietario:
/// *"rehaz numeros e historial al estilo inicio, pero todo mejor"*. Arriba el
/// resumen (cuantas versiones, cuantas permanentes); debajo la cadena como un
/// rail con un rombo por version y una tarjeta al lado: el numero en grande,
/// cuando, y lo que importa de verdad --si es la de ahora, si tiene nombre
/// (permanente) o si es automatica (el recolector puede soltarla)--.
///
/// `desde` es la primera que se ve; si la marcada se sale por abajo, la vista
/// baja con ella (antes se quedaba marcando una caja que no se veia).
pub(crate) fn paint(
    p: &bmo::Pantalla,
    z: &Zona,
    desde: usize,
    sel: usize,
    borde: u32,
    fondo: u32,
    acento: u32,
    sel_fondo: u32,
) {
    if !z.hay() {
        return;
    }
    let neon = super::data::DATA_TITLE;
    let cuantas = bmo::estratos::hist_cuantas() as usize;
    let ancho = z.w.saturating_sub(24).min(ANCHO_MAX);
    let x0 = z.x + 12;

    // -- el encabezado --
    let y0 = z.y + 12;
    super::borde::marco(p, x0, y0, ancho, CABEZA_H, super::RADIUS, borde, fondo);
    p.rect(x0 + super::RADIUS, y0, ancho.saturating_sub(2 * super::RADIUS), 1, neon);
    super::iconos::vector(p, x0 + 14, y0 + 12, 40, &super::iconos::dibujos::ESTRATOS, &super::iconos::paleta(neon, neon), fondo);
    p.texto_escala(x0 + 66, y0 + 10, "Historial", INK, 2);
    if cuantas == 0 {
        p.texto(x0 + 66, y0 + 44, "no hay historia que mostrar: el volumen no monta, o no tiene ni un estrato.", INK_BAD_O_DIM);
        return;
    }
    let permanentes = (0..cuantas as u64).filter(|&i| bmo::estratos::hist_con_nombre(i)).count();
    let mut b = [0u8; 10];
    let ry = y0 + 44;
    let n = crate::text::decimal(cuantas as u64, &mut b);
    let x = p.texto_bytes(x0 + 66, ry, &b[..n], neon);
    let x = p.texto(x, ry, " versiones   ", INK_DIM);
    let n = crate::text::decimal(permanentes as u64, &mut b);
    let x = p.texto_bytes(x, ry, &b[..n], INK_OK);
    let x = p.texto(x, ry, " permanentes   ", INK_DIM);
    if bmo::estratos::hist_recortada() {
        p.texto(x, ry, "(y mas atras, fuera de lo que se guarda)", INK_DIM);
    }
    let pista = "ENTRAR vuelve a la marcada: no se pierde nada";
    let px = (x0 + ancho).saturating_sub(16 + pista.len() as u32 * bmo::GLIFO_ANCHO);
    if px > x0 + 360 {
        p.texto(px, y0 + 16, pista, INK_DIM);
    }

    // -- la cadena --
    let ty = y0 + CABEZA_H + 14;
    let paso = CAJA_H + HUECO;
    let caben = ((z.y + z.h).saturating_sub(ty + 20) / paso).max(1) as usize;
    let desde = desde.max((sel + 1).saturating_sub(caben)).min(cuantas.saturating_sub(1));
    let hasta = (desde + caben).min(cuantas);
    let rail = x0 + RAIL;
    let cx = rail + 26;
    let cw = ancho.saturating_sub(cx - x0);
    // El rail entero de las que se ven, y que sigue si hay mas.
    let alto_rail = (hasta - desde) as u32 * paso;
    p.rect(rail, ty, 2, alto_rail.saturating_sub(HUECO), borde);

    let mut y = ty;
    for i in desde..hasta {
        let es_ahora = i == 0;
        let tiene_nombre = bmo::estratos::hist_con_nombre(i as u64);
        let marcada = i == sel;
        // El rombo en el rail: lleno para la de ahora, verde si es permanente.
        let ry = y + CAJA_H / 2;
        let c_rombo = if es_ahora { neon } else if tiene_nombre { INK_OK } else { borde };
        for k in 0..5u32 {
            let w = 2 * (5 - k) - 1;
            p.rect(rail + 1 - (w / 2), ry - k, w, 1, c_rombo);
            p.rect(rail + 1 - (w / 2), ry + k, w, 1, c_rombo);
        }
        // La tarjeta.
        let filo = if marcada { acento } else if tiene_nombre { INK_OK } else { borde };
        let cuerpo = if marcada { sel_fondo } else { fondo };
        super::borde::marco(p, cx, y, cw, CAJA_H, super::RADIUS, filo, cuerpo);
        // El numero de version, en grande: la mas nueva es la mas alta.
        let mut v = [0u8; 12];
        v[0] = b'v';
        let n = crate::text::decimal((cuantas - i) as u64, &mut b);
        v[1..1 + n].copy_from_slice(&b[..n]);
        let vs = core::str::from_utf8(&v[..1 + n]).unwrap_or("v?");
        p.texto_escala(cx + 14, y + (CAJA_H - 2 * bmo::GLIFO_ALTO) / 2, vs, if es_ahora { neon } else { INK }, 2);
        // Cuando.
        let tx = cx + 120;
        let l1 = y + 8;
        match bmo_rtc::desempaquetar(bmo::estratos::hist_cuando(i as u64)) {
            Some(f) => {
                let mut fb = [0u8; 24];
                let n = bmo_rtc::escribir(&f, &mut fb);
                p.texto_bytes(tx, l1, &fb[..n.min(19)], INK);
            }
            // ** No se inventa una fecha: 1970 mentiria con mas conviccion.
            None => {
                p.texto(tx, l1, "sin fechar", INK_DIM);
            }
        }
        // Que es: la de ahora, permanente con su nombre, o automatica.
        let l2 = l1 + bmo::GLIFO_ALTO + 6;
        let mut x = tx;
        if es_ahora {
            let t = "la de ahora";
            let w = (t.len() as u32 + 2) * bmo::GLIFO_ANCHO;
            super::borde::marco(p, x, l2 - 2, w, bmo::GLIFO_ALTO + 4, 6, neon, cuerpo);
            p.texto(x + bmo::GLIFO_ANCHO, l2, t, neon);
            x += w + 10;
        }
        if tiene_nombre {
            let mut nom = [0u8; 64];
            let n = bmo::estratos::hist_nombre(i as u64, &mut nom);
            let x2 = p.texto(x, l2, "permanente: ", INK_OK);
            p.texto_bytes(x2, l2, &nom[..n], INK_OK);
        } else {
            p.texto(x, l2, "automatica -- el recolector puede soltarla", INK_DIM);
        }
        // Quien la hizo, a la derecha.
        let pid = bmo::estratos::hist_quien(i as u64);
        let n = crate::text::decimal(pid, &mut b);
        let qx = (cx + cw).saturating_sub(16 + (4 + n as u32) * bmo::GLIFO_ANCHO);
        if qx > tx + 380 {
            let x = p.texto(qx, l1, "pid ", INK_DIM);
            p.texto_bytes(x, l1, &b[..n], 0x00C9_D8CF);
        }
        y += paso;
    }
    if hasta < cuantas {
        p.texto(cx, y + 2, "y mas abajo... (flechas)", INK_DIM);
    }
}

/// El rojo apagado de "aqui no hay nada que mostrar". No es un error del disco,
/// asi que no lleva el rojo de alarma.
const INK_BAD_O_DIM: u32 = 0x009A_96B8;
