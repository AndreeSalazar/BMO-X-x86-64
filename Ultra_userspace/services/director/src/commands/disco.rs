//! **`disco`: la terminal de administracion del almacen.**
//!
//! [consumo] APARATO   `trim_libre` le MANDA al SSD que borre bloques. El
//!                     disco se queda distinto, y eso es lo que distingue un
//!                     informe de una orden (L6h)
//!           [!] MEZCLA -- declara la PEOR (L6h). Casi todo es el informe del
//!                         disco, que solo LEE; lo que gasta es UNA orden,
//!                         `trim`, que le manda al SSD borrar bloques. Leer y
//!                         mandar no cuestan lo mismo, y aqui van juntos.
//!
//! === Por que esto existe, y por que aqui ===
//!
//! Porque BMO-X no es Linux, no es Windows y no es un Mac: no hay `fstrim`, no
//! hay `hdparm`, no hay un `/dev` donde apuntar una herramienta ajena. Lo que
//! el sistema sepa hacer con su disco **tiene que poder pedirse desde donde vive
//! el propietario**, que es este escritorio -- al shell de Ring 0 no se vuelve una vez
//! el compositor reclama la entrada, y una orden que solo existe alli es codigo
//! que su propietario no puede usar. Ya paso con `smp`, con `ext` y con `audio`.
//!
//! === La regla de esta caja: PROPONER y luego obedecer ===
//!
//! La seccion 9 de ESTRATOS lo pide con todas las letras -- *"un mando manual
//! con lo que va a soltar listado antes de hacerlo"*. Por eso:
//!
//! ```text
//!   disco trim       MUESTRA la propuesta y NO manda nada
//!   disco trim ya    la manda
//! ```
//!
//! No es una confirmacion de cortesia: recortar es **destructivo**, y una orden
//! que se ejecuta en el momento en que se teclea no deja sitio para leerla.
//!
//! === Lo que esta caja NO tiene, y no es un olvido ===
//!
//! Una forma de decir **donde**. Ni `disco trim <lba>` ni nada que se le
//! parezca: el rango lo calcula el kernel y lo comprueba contra la ventana de
//! escritura. Un recorte apuntable desde el teclado seria un borrado a distancia
//! con formulario, y en esta maquina el vecino de particion es el arranque.

use bmo_userland as bmo;

use super::tabla::{campo, section};
use super::After;
use crate::desktop::Desktop;
use crate::scene::output::{Output, INK_ECHO, INK_ERR, INK_GOOD, INK_PLAIN};
use crate::scene::{paint_status, INK_DIM};
use crate::paint_output;

/// Sectores que cubre un bloque de payload: 64 descriptores de 65.535 sectores.
///
/// ** Es del FORMATO de `DATA SET MANAGEMENT`, no del disco ni de esta ventana:
/// cuantos bloques caben en una orden lo dice el aparato
/// (`INFO_DISCO_TRIM_BLOQUES`, la palabra 105) y se pregunta. Multiplicar los
/// dos da el numero REAL de ordenes, no un techo inventado en este lado.
const SECTORES_POR_BLOQUE_DE_PAYLOAD: u64 = 64 * 65_535;

/// ** EL DISCO: lo que CONTESTA, y luego lo que se concluye de ello.
///
/// El orden no es decorativo. Primero los hechos --gira, el cable, la
/// geometria-- y **despues** el veredicto, porque asi se puede estar en
/// desacuerdo con la conclusion sin perder la evidencia. Un veredicto que
/// aparece sin lo que lo sostiene no se puede discutir, solo creer.
///
/// ** Y la primera linea es la que hasta el 2026-08-17 no existia: BMO-X le
/// preguntaba al disco modelo, serie y capacidad, y **no sabia si giraba** --
/// mientras el esquema de ESTRATOS razonaba sobre TRIM y la ley sobre colas.
/// Ver `docs/componente/EL_DISCO_EXIGE.md`.
///
/// ** Es `pub(crate)` porque lo pintan tambien `save` (informe/DISCO.TXT) y la
/// vista de ficheros. Copiarlo alli habria dado dos tablas del mismo aparato
/// que se separan a la tercera vez que alguien toca una. Vivia en `reports.rs`
/// y se mudo aqui el 23-09 (L6a): es la tabla de ESTE aparato.
#[inline(never)]
pub(crate) fn report_disco(s: &mut Output) {
    let medio = bmo::info(bmo::INFO_DISCO_MEDIO);
    let enlace = bmo::info(bmo::INFO_DISCO_ENLACE);
    let geo = bmo::info(bmo::INFO_DISCO_GEOMETRIA);
    let juicio = bmo::info(bmo::INFO_DISCO_JUICIO);

    // Sin foto no se inventa nada: se dice que no la hay y se sale.
    if medio == 0 && enlace == 0 && geo == 0 {
        campo(s, b"identify");
        s.with_ink(INK_ERR);
        s.text(b"este kernel no lee las palabras del disco (o el IDENTIFY fallo)\n");
        s.with_ink(INK_PLAIN);
        return;
    }

    // -- EL MEDIO. Una palabra, y la frase SOLO cuando dice algo raro.
    let clase = (medio >> bmo::DISCO_MEDIO_CLASE_SHIFT) & bmo::DISCO_MEDIO_CLASE_MASK;
    let rpm = (medio >> bmo::DISCO_MEDIO_RPM_SHIFT) & bmo::DISCO_MEDIO_RPM_MASK;
    campo(s, b"medium");
    match clase {
        bmo::DISCO_MEDIO_NO_ROTA => {
            s.with_ink(INK_GOOD);
            s.text(b"SSD");
        }
        bmo::DISCO_MEDIO_ROTA => {
            s.text(b"HDD, ");
            s.dec(rpm);
            s.text(b" rpm   el ORDEN de los sectores manda");
        }
        bmo::DISCO_MEDIO_NO_CONTESTA => {
            s.with_ink(INK_ERR);
            s.text(b"el disco NO DICE si gira (217 = 0) -- no se asume nada");
        }
        _ => {
            s.with_ink(INK_ERR);
            s.text(b"la palabra 217 trae un valor RESERVADO: ");
            s.hex(medio & bmo::DISCO_MEDIO_CRUDO_MASK, 4);
        }
    }
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');

    // -- EL CABLE. `soportado / negociado`, y nada mas cuando cuadran.
    campo(s, b"link");
    let mejor = if enlace & bmo::DISCO_ENLACE_GEN3 != 0 { 3 }
        else if enlace & bmo::DISCO_ENLACE_GEN2 != 0 { 2 }
        else if enlace & bmo::DISCO_ENLACE_GEN1 != 0 { 1 } else { 0 };
    let nego = (enlace >> bmo::DISCO_ENLACE_NEGOCIADA_SHIFT)
        & bmo::DISCO_ENLACE_NEGOCIADA_MASK;
    s.text(b"SATA Gen");
    s.dec(mejor);
    if nego == 0 {
        s.text(b" / el disco no dice a que va");
    } else {
        s.text(b" / Gen");
        s.dec(nego);
    }
    if juicio & bmo::DISCO_JUICIO_ENLACE_BAJO != 0 {
        s.with_ink(INK_ERR);
        s.text(b"   POR DEBAJO");
        s.with_ink(INK_PLAIN);
    }
    s.byte(b'\n');

    // -- ** EL OTRO EXTREMO DEL CABLE: la controladora y su puerto (P0, 23-09).
    //
    // El `CAP` vivia en un comentario de `arranque.rs`. Se descifra aqui con los
    // nombres del estandar AHCI y el crudo va al lado: el registro es la prueba.
    let hba = bmo::info(bmo::INFO_DISCO_HBA);
    if hba & bmo::DISCO_HBA_HAY != 0 {
        let cap = hba & bmo::DISCO_HBA_CAP_MASK;
        campo(s, b"hba");
        s.dec(((cap >> 8) & 0x1F) + 1); // NCS, con su -1
        s.text(b" ranuras");
        s.text(if cap & (1 << 30) != 0 { b", NCQ" as &[u8] } else { b", SIN NCQ" });
        s.text(b", hasta Gen");
        s.dec((cap >> 20) & 0xF); // ISS
        if cap & (1 << 31) != 0 {
            s.text(b", DMA 64 bits"); // S64A
        }
        s.text(b"   CAP 0x");
        s.hex(cap, 8);
        s.byte(b'\n');

        let ssts = (hba >> bmo::DISCO_HBA_SSTS_SHIFT) & bmo::DISCO_HBA_SSTS_MASK;
        let spd = (ssts >> 4) & 0xF;
        campo(s, b"port");
        s.dec((hba >> bmo::DISCO_HBA_PUERTO_SHIFT) & bmo::DISCO_HBA_PUERTO_MASK);
        match ssts & 0xF {
            3 => {
                s.text(b": enlace Gen");
                s.dec(spd);
            }
            1 => s.text(b": algo conectado SIN comunicacion"),
            _ => s.text(b": sin enlace"),
        }
        // ** Los dos extremos del mismo cable: el HBA dice SPD, el disco dice la
        // palabra 77. Si discrepan, eso es un hallazgo, no un redondeo.
        if nego != 0 && spd != 0 && spd != nego {
            s.with_ink(INK_ERR);
            s.text(b"   y el disco dice Gen");
            s.dec(nego);
            s.with_ink(INK_PLAIN);
        }
        s.text(match (ssts >> 8) & 0xF {
            1 => b"   activo" as &[u8],
            2 => b"   dormido (Partial)",
            6 => b"   dormido (Slumber)",
            8 => b"   dormido (DevSleep)",
            _ => b"",
        });
        s.text(b"   SSTS 0x");
        s.hex(ssts, 3);
        s.byte(b'\n');
    }

    // -- ** LA COLA. La resta que dice cuanto del aparato esta parado.
    let cola = (enlace >> bmo::DISCO_ENLACE_COLA_SHIFT) & bmo::DISCO_ENLACE_COLA_MASK;
    let usadas = (enlace >> bmo::DISCO_ENLACE_USADAS_SHIFT) & bmo::DISCO_ENLACE_USADAS_MASK;
    let ociosas = (enlace >> bmo::DISCO_ENLACE_OCIOSAS_SHIFT) & bmo::DISCO_ENLACE_OCIOSAS_MASK;
    campo(s, b"queue");
    s.dec(usadas);
    s.text(b" de ");
    s.dec(cola);
    if enlace & bmo::DISCO_ENLACE_NCQ == 0 {
        s.text(b"   (sin NCQ)");
    } else if ociosas > 0 {
        s.with_ink(INK_ERR);
        s.text(b"   ");
        s.dec(ociosas);
        s.text(b" PARADAS");
        s.with_ink(INK_PLAIN);
    }
    s.byte(b'\n');

    // -- LA GEOMETRIA. El exponente, no una cuenta.
    campo(s, b"sector");
    if geo & bmo::DISCO_GEO_106_VALIDA == 0 {
        s.text(b"sin declarar (palabra 106 sin guarda)");
    } else {
        let exp = geo & bmo::DISCO_GEO_EXP_MASK;
        s.dec(512u64 << exp);
        s.text(b" B fisico");
        if exp > 0 {
            s.text(b" = ");
            s.dec(1u64 << exp);
            s.text(b" logicos");
        }
        if geo & bmo::DISCO_GEO_209_VALIDA != 0 {
            let d = (geo >> bmo::DISCO_GEO_DESPL_SHIFT) & bmo::DISCO_GEO_DESPL_MASK;
            s.text(b", LBA 0 desplazado ");
            s.dec(d);
        }
    }
    s.byte(b'\n');

    // -- EL VEREDICTO, y va detras de sus hechos a proposito.
    campo(s, b"profile");
    if juicio & bmo::DISCO_JUICIO_HAY_PERFIL == 0 {
        s.with_ink(INK_ERR);
        s.text(b"NINGUNO para este disco -- se toma el camino conservador");
    } else {
        s.with_ink(INK_GOOD);
        s.text(b"reconocido");
        s.with_ink(INK_PLAIN);
        if juicio & bmo::DISCO_JUICIO_MEDIDO == 0 {
            s.text(b"   cifras de CATALOGO, no medidas");
        }
    }
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');

    // -- TRIM, y al lado lo que cabe en una orden: son la misma pregunta.
    campo(s, b"trim");
    if juicio & bmo::DISCO_JUICIO_SOLIDO_SIN_TRIM != 0 {
        s.with_ink(INK_ERR);
        s.text(b"NO -- y el medio es solido: el recolector no puede avisar");
    } else if juicio & bmo::DISCO_JUICIO_TRIM != 0 {
        s.with_ink(INK_GOOD);
        s.text(b"si");
        s.with_ink(INK_PLAIN);
        s.text(b"   ");
        s.dec(bmo::info(bmo::INFO_DISCO_TRIM_BLOQUES));
        s.text(b" bloque(s) por orden");
    } else {
        s.text(b"no (y el medio no lo necesita)");
    }
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');

    // -- ** LA CACHE DE ESCRITURA (palabras 82-85). Va ANTES de la barrera
    // porque es lo que la decide: con la cache apagada, un OK ya es la NAND.
    let cache = bmo::info(bmo::INFO_DISCO_CACHE);
    campo(s, b"cache");
    if cache & bmo::DISCO_CACHE_VALIDA == 0 {
        s.text(b"el disco no rellena las palabras 82-87: no se sabe");
    } else if cache & bmo::DISCO_CACHE_SOPORTADA == 0 {
        s.with_ink(INK_GOOD);
        s.text(b"sin cache de escritura: lo que vuelve OK ya esta en la NAND");
    } else if cache & bmo::DISCO_CACHE_ENCENDIDA != 0 {
        s.text(b"ENCENDIDA (85): un OK es 'aceptado', no 'guardado'");
    } else {
        s.with_ink(INK_GOOD);
        s.text(b"soportada y APAGADA (85): un OK ya es la NAND");
    }
    s.with_ink(INK_PLAIN);
    if cache & bmo::DISCO_CACHE_FLUSH_EXT != 0 {
        s.text(b", FLUSH EXT");
    }
    if cache & bmo::DISCO_CACHE_FUA != 0 {
        s.text(b", FUA");
    }
    s.byte(b'\n');

    // ** La linea que no puede faltar el dia que se escriba de verdad.
    campo(s, b"barrier");
    if juicio & bmo::DISCO_JUICIO_SOLO_BARRERA != 0 {
        s.with_ink(INK_ERR);
        s.text(b"el FLUSH CACHE es LO UNICO: no termina lo que empezo");
    } else {
        s.with_ink(INK_GOOD);
        s.text(b"tiene con que terminar un corte de corriente");
    }
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');

    campo(s, b"align");
    let frontera = (juicio >> bmo::DISCO_JUICIO_FRONTERA_SHIFT)
        & bmo::DISCO_JUICIO_FRONTERA_MASK;
    if frontera == 0 {
        s.with_ink(INK_ERR);
        s.text(b"NO SE PUEDE: el bloque de borrado no se le pregunta a un disco");
    } else {
        s.dec(frontera);
        s.text(b" KiB   del perfil, no leido");
    }
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');

    if juicio & bmo::DISCO_JUICIO_DESALINEADO != 0 {
        campo(s, b"AVISO");
        s.with_ink(INK_ERR);
        s.text(b"LBA 0 no cae en frontera fisica: cada escritura paga dos sectores\n");
        s.with_ink(INK_PLAIN);
    }

    // -- ** EL HILO DEL DISCO (D1, 23-09): si trae los ficheros durmiendo.
    let h = bmo::info(bmo::INFO_DISCO_HILO);
    campo(s, b"thread");
    if h & bmo::DISCO_HILO_VIVO == 0 {
        s.with_ink(INK_ERR);
        s.text(b"sin hilo: los ficheros se traen GIRANDO dentro del syscall");
    } else {
        let vuelos = h & bmo::DISCO_HILO_VUELOS_MASK;
        let ajenas = (h >> bmo::DISCO_HILO_AJENAS_SHIFT) & bmo::DISCO_HILO_AJENAS_MASK;
        let irq = (h >> bmo::DISCO_HILO_IRQ_SHIFT) & bmo::DISCO_HILO_IRQ_MASK;
        s.with_ink(INK_GOOD);
        s.dec(vuelos);
        s.text(b" ordenes en vuelo");
        s.with_ink(INK_PLAIN);
        s.text(b"   ");
        s.dec(irq);
        s.text(b" despertares por la IRQ   ");
        s.dec(ajenas);
        s.text(b" terminadas por otro");
        // Con ordenes y sin un solo aviso: la placa no enruta la IRQ y el hilo
        // vive de su red de 2 ms. Funciona, pero no es lo que se prometio.
        if vuelos > 0 && irq == 0 {
            s.with_ink(INK_ERR);
            s.text(b"   SIN IRQ: vive de la red de 2 ms");
        }
    }
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');

    // -- ** EL METRO (D0, 23-09): la unica cifra de velocidad que es de ESTE
    // disco. Sin medir se dice, y se dice como medirlo.
    if bmo::info(bmo::INFO_DISCO_BANDA) == 0 {
        campo(s, b"read");
        s.with_ink(INK_ECHO);
        s.text(b"sin medir en esta sesion   (disco banda)\n");
        s.with_ink(INK_PLAIN);
    } else {
        detalle_banda(s);
    }
}

/// El cuadro entero: que aparato es, cuanto queda y que se le ha devuelto.
pub(crate) fn cuadro(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    let s = &mut dsk.out.grid;
    section(s, b"disco");
    report_disco(s);
    espacio(s);
    devuelto(s);
    ordenes(s);
    paint_status(p, &dsk.run_box, "disco", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

/// **Cuanto queda en el volumen**, que es la pregunta previa a cualquier otra.
///
/// Los numeros son los de `bmo_estratos::espacio` y los umbrales tambien: aqui
/// no se decide donde cae el ambar. La cuenta es **una resta** porque ESTRATOS
/// reserva con un puntero que solo avanza -- ni mapa de bits ni fragmentacion.
fn espacio(s: &mut Output) {
    section(s, b"volume");
    if bmo::info(bmo::INFO_ES_MONTADO) == 0 {
        campo(s, b"estratos");
        s.with_ink(INK_ERR);
        s.text(b"ningun volumen montado: no hay espacio del que hablar\n");
        s.with_ink(INK_PLAIN);
        return;
    }
    let bloques = bmo::info(bmo::INFO_ES_BLOQUES);
    let usados = bmo::info(bmo::INFO_ES_USADOS);
    let tam = bmo::info(bmo::INFO_ES_BLOQUE_TAM).max(1);

    campo(s, b"estratos");
    s.with_ink(INK_GOOD);
    s.text(b"gen ");
    s.dec(bmo::info(bmo::INFO_ES_GENERACION));
    s.with_ink(INK_PLAIN);
    // La identidad va pegada: un volumen clonado se monta y se lee igual, y la
    // diferencia es que NO tiene ventana de escritura. Sin esta linea,
    // "montado" se lee como "listo para todo".
    if bmo::info(bmo::INFO_ES_IDENTIDAD) == 0 {
        s.with_ink(INK_ERR);
        s.text(b"   NO nacio en este disco (clonado?)");
        s.with_ink(INK_PLAIN);
    } else {
        s.text(b"   de este disco");
    }
    s.byte(b'\n');

    campo(s, b"used");
    s.size(usados.saturating_mul(tam));
    s.text(b" de ");
    s.size(bloques.saturating_mul(tam));
    s.text(b"   ");
    s.pct(usados, bloques);
    s.byte(b'\n');

    campo(s, b"blocks");
    s.dec(usados);
    s.text(b" de ");
    s.dec(bloques);
    s.text(b"   de ");
    s.dec(tam / 1024);
    s.text(b" KiB\n");

    campo(s, b"level");
    let nivel = bmo::info(bmo::INFO_ES_NIVEL);
    s.with_ink(if nivel == 0 { INK_GOOD } else { INK_ERR });
    s.text(match nivel {
        0 => b"holgado" as &[u8],
        1 => b"AVISO: por encima del 70%",
        2 => b"FAULT: por encima del 85%",
        _ => b"SOLO LECTURA: por encima del 95%",
    });
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');

    campo(s, b"write");
    if bmo::info(bmo::INFO_ES_ESCRIBIBLE) != 0 {
        s.with_ink(INK_GOOD);
        s.text(b"si");
    } else {
        s.with_ink(INK_ERR);
        s.text(b"NO: sin esto no hay sellado ni recorte");
    }
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');
}

/// **Lo que ya se le devolvio al aparato.** Cero significa *nadie lo ha pedido*.
fn devuelto(s: &mut Output) {
    let sectores = bmo::info(bmo::INFO_DISCO_TRIM_SECTORES);
    let ordenes = bmo::info(bmo::INFO_DISCO_TRIM_ORDENES);
    campo(s, b"trimmed");
    if sectores == 0 {
        s.with_ink(INK_ECHO);
        s.text(b"nada en esta sesion   (lo pide una persona)");
        s.with_ink(INK_PLAIN);
    } else {
        s.size(sectores.saturating_mul(512));
        s.text(b"   en ");
        s.dec(ordenes);
        s.text(b" ordenes");
    }
    s.byte(b'\n');
    // ** Y el ultimo fallo, si lo hubo, EN LA MISMA TABLA. Un recorte que fallo
    // hace un rato y no deja rastro en `disco` obliga a repetirlo para volver a
    // ver el motivo -- y repetir es justo lo que no se debe hacer con la unica
    // orden destructiva de la caja.
    fallo(s);
}

/// El ultimo fallo del recorte, con **el numero del aparato al lado**.
fn fallo(s: &mut Output) {
    let v = bmo::info(bmo::INFO_DISCO_TRIM_FALLO);
    if v == 0 {
        return;
    }
    let clase = v >> bmo::DISCO_FALLO_CLASE_SHIFT;
    let tfd = v & bmo::DISCO_FALLO_TFD_MASK;
    campo(s, b"fallo");
    s.with_ink(INK_ERR);
    s.text(bmo::fallo_en_palabras(clase));
    s.with_ink(INK_PLAIN);
    if clase == bmo::DISCO_FALLO_APARATO {
        // El PxTFD CRUDO y luego los bits que se saben leer: el byte es la
        // prueba y las palabras son la opinion. Mismo trato que el PHYstatus.
        s.text(b"\n");
        campo(s, b"PxTFD");
        s.text(b"0x");
        s.hex(tfd, 8);
        let err = (tfd >> 8) & 0xFF;
        if err & 0x04 != 0 {
            s.text(b"   ABRT: el disco NO CONOCE esa orden");
        } else if err & 0x10 != 0 {
            s.text(b"   IDNF: ese sector no");
        } else if err != 0 {
            s.text(b"   error 0x");
            s.hex(err, 2);
        }
    }
    s.byte(b'\n');
}

/// Las ordenes de esta caja. Van al final de `disco` a secas, que es donde uno
/// se pregunta "y ahora que puedo hacer con esto?".
fn ordenes(s: &mut Output) {
    s.with_ink(INK_ECHO);
    s.text(b"    disco trim   propone el recorte    trim ya   lo manda\n");
    s.text(b"    disco espacio / barrera\n");
    s.text(b"    disco banda [MiB]   MIDE la lectura (solo lee)\n");
    s.with_ink(INK_PLAIN);
}

/// Solo el espacio.
pub(crate) fn solo_espacio(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    espacio(&mut dsk.out.grid);
    paint_status(p, &dsk.run_box, "espacio", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

/// **La propuesta**: que se recortaria, cuanto es, y por que no se pierde nada.
///
/// === Los numeros son LOS DE LA ORDEN, no unos parecidos ===
///
/// El rango se pide con `INFO_DISCO_COLA_LBA` y `..._SECTORES`, y al otro lado
/// esos dos campos los sirve **la misma funcion del kernel que ejecuta el
/// recorte**. La primera version los deducia aqui de `INFO_ES_BLOQUES`,
/// `INFO_ES_USADOS` y `INFO_ES_BLOQUE_TAM` -- una cuenta paralela que hoy da lo
/// mismo y que el dia que una de las dos cambie **muestra un rango y recorta
/// otro**. Una propuesta que no es exactamente la orden no es una propuesta.
///
/// [!] Sigue sin llamar al disco: son campos de informe. Una propuesta que
/// tuviera que tocar el aparato para poder ensenarse ya lo habria tocado.
fn propuesta(s: &mut Output, explica: bool) -> bool {
    section(s, b"recorte: la propuesta");
    if bmo::info(bmo::INFO_ES_MONTADO) == 0 {
        s.with_ink(INK_ERR);
        s.text(b"    sin volumen ESTRATOS montado no hay cola libre que devolver\n");
        s.with_ink(INK_PLAIN);
        return false;
    }
    // ** Y ANTES DE NADA: lo que el disco dijo. Proponer un recorte a un aparato
    // que no declara TRIM seria mostrar un plan que se va a rechazar solo.
    let juicio = bmo::info(bmo::INFO_DISCO_JUICIO);
    if juicio & bmo::DISCO_JUICIO_TRIM == 0 {
        s.with_ink(INK_ERR);
        s.text(b"    este disco NO declara TRIM (palabra 169): no hay nada que mandar\n");
        s.with_ink(INK_PLAIN);
        return false;
    }

    let lba = bmo::info(bmo::INFO_DISCO_COLA_LBA);
    let sectores = bmo::info(bmo::INFO_DISCO_COLA_SECTORES);
    if sectores == 0 {
        s.with_ink(INK_ERR);
        s.text(b"    la cola libre esta vacia: el volumen esta lleno\n");
        s.with_ink(INK_PLAIN);
        return false;
    }

    campo(s, b"free tail");
    s.size(sectores.saturating_mul(512));
    s.text(b"   desde el bloque ");
    s.dec(bmo::info(bmo::INFO_ES_USADOS));
    s.byte(b'\n');

    campo(s, b"sectors");
    s.dec(sectores);
    s.text(b" de 512 B   desde el LBA ");
    s.dec(lba);
    s.byte(b'\n');

    // El numero REAL: lo que cabe en una orden lo dice el disco (palabra 105) y
    // se pregunta, en vez de suponer el minimo y decir "como mucho".
    let por_orden = bmo::info(bmo::INFO_DISCO_TRIM_BLOQUES).max(1)
        .saturating_mul(SECTORES_POR_BLOQUE_DE_PAYLOAD);
    campo(s, b"orders");
    s.dec(sectores.div_ceil(por_orden));
    s.text(b"   (el disco admite ");
    s.dec(bmo::info(bmo::INFO_DISCO_TRIM_BLOQUES));
    s.text(b" bloque(s) por orden)\n");

    // ** LA FRASE QUE JUSTIFICA QUE ESTO SEA SEGURO, y va en la propuesta y no
    // en un README: es lo que el que va a teclear `ya` necesita saber.
    //
    // Y va SOLO cuando se propone. Al ejecutar se repetia entera, o sea tres
    // renglones identicos a los de hace dos segundos -- y lo que el que mira
    // busca en ese momento es el resultado, no el argumento que ya leyo.
    if explica {
        s.with_ink(INK_ECHO);
        s.text(b"    no se pierde nada: por encima de log_head no llega ningun\n");
        s.text(b"    estrato, y ese puntero solo avanza. NO es el recolector.\n");
        s.with_ink(INK_PLAIN);
    }
    true
}

/// `disco trim` -- la propuesta, y como pedirla.
pub(crate) fn trim_propuesta(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    if propuesta(&mut dsk.out.grid, true) {
        dsk.out.grid.with_ink(INK_GOOD);
        dsk.out.grid.text(b"    escribe `disco trim ya` para mandarlo\n");
        dsk.out.grid.with_ink(INK_PLAIN);
    }
    paint_status(p, &dsk.run_box, "trim: propuesta", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

/// `disco trim <algo que no es "ya">`.
///
/// Se muestra la propuesta igual --no toca nada-- y se dice **cual era la palabra
/// buena**. Contestar "no lo conozco" a alguien que ya escribio `trim` seria
/// mandarle a `help` teniendo la orden medio escrita.
pub(crate) fn trim_argumento(dsk: &mut Desktop, p: &bmo::Pantalla, que: &[u8]) -> After {
    let s = &mut dsk.out.grid;
    s.with_ink(INK_ERR);
    s.text(b"  `");
    s.text(que);
    s.text(b"` no significa nada detras de `trim`\n");
    s.with_ink(INK_PLAIN);
    if propuesta(&mut dsk.out.grid, true) {
        dsk.out.grid.with_ink(INK_GOOD);
        dsk.out.grid.text(b"    la palabra es `ya`:  disco trim ya\n");
        dsk.out.grid.with_ink(INK_PLAIN);
    }
    paint_status(p, &dsk.run_box, "trim: propuesta", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

/// `disco trim ya` -- **la orden de verdad**.
///
/// El aviso se pinta y se VUELCA antes de llamar, igual que en `smp`: la
/// llamada no vuelve hasta que el disco ha tragado cientos de ordenes, y un
/// mensaje escrito despues no explica nada -- para entonces la espera ya paso y
/// lo que el propietario habria visto es un escritorio congelado sin motivo.
pub(crate) fn trim_ya(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    if !propuesta(&mut dsk.out.grid, false) {
        paint_status(p, &dsk.run_box, "trim", INK_DIM);
        dsk.field.n = 0;
        return After::Settle;
    }
    dsk.out.grid.text(b"    mandando el recorte (esto tarda)...\n");
    paint_output(p, &dsk.run_box, &dsk.out.grid);
    p.volcar();

    let (motivo, sectores) = bmo::trim_libre();
    let s = &mut dsk.out.grid;
    match motivo {
        bmo::DISCO_TRIM_HECHO => {
            s.with_ink(INK_GOOD);
            s.text(b"    DEVUELTO: ");
            s.size(sectores.saturating_mul(512));
            s.with_ink(INK_PLAIN);
            s.text(b"   en ");
            s.dec(bmo::info(bmo::INFO_DISCO_TRIM_ORDENES));
            s.text(b" ordenes (total de la sesion)\n");
            // La barrera la manda el kernel detras del recorte; decirlo aqui es
            // lo que separa "el disco lo acepto" de "el disco lo asumio".
            s.with_ink(INK_ECHO);
            s.text(b"    con FLUSH CACHE detras: este disco no tiene condensadores\n");
            s.with_ink(INK_PLAIN);
        }
        // ** El fallo lleva lo que SI se hizo. Un recorte a medias no se
        // deshace, y sin este numero el que mire creeria que no paso nada.
        // ** EL MOTIVO SE PINTA AQUI, y esa es la leccion del 17-08.
        //
        // Antes decia "el disco RECHAZO la orden" y mandaba a F11. Las dos
        // mitades estaban mal: **no siempre rechaza** --puede no contestar a
        // tiempo, que acusa al driver y no al aparato-- y mandar a otra ventana
        // por el numero es pedir un viaje mas cuando el que mira ya esta aqui.
        bmo::DISCO_TRIM_FALLO => {
            s.with_ink(INK_ERR);
            s.text(b"    NO SE PUDO: ");
            let v = bmo::info(bmo::INFO_DISCO_TRIM_FALLO);
            s.text(bmo::fallo_en_palabras(v >> bmo::DISCO_FALLO_CLASE_SHIFT));
            s.with_ink(INK_PLAIN);
            s.byte(b'\n');
            fallo(s);
            if sectores > 0 {
                s.text(b"    se devolvio antes de romperse: ");
                s.size(sectores.saturating_mul(512));
                s.byte(b'\n');
            }
        }
        otro => {
            s.with_ink(INK_ERR);
            s.text(b"    no se mando: ");
            s.text(bmo::motivo_en_palabras(otro));
            s.byte(b'\n');
            s.with_ink(INK_PLAIN);
        }
    }
    paint_status(p, &dsk.run_box, "trim", INK_DIM);
    dsk.field.n = 0;
    dsk.field.cur = 0;
    After::NextKey
}

/// `disco barrera` -- el `FLUSH CACHE`, a mano.
pub(crate) fn barrera(dsk: &mut Desktop, p: &bmo::Pantalla) -> After {
    let ok = bmo::barrera();
    let s = &mut dsk.out.grid;
    campo(s, b"barrier");
    if ok {
        s.with_ink(INK_GOOD);
        s.text(b"el disco bajo al plato lo que tenia aceptado");
    } else {
        s.with_ink(INK_ERR);
        s.text(b"NO: sin disco, o la escritura no esta armada");
    }
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');
    paint_status(p, &dsk.run_box, "barrera", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

/// **`disco banda [MiB]` -- EL METRO: cuanto lee ESTE disco, medido aqui.**
///
/// === Por que existe (paso D0 del plan del disco, 23-09) ===
///
/// La LEY 24: una cifra de la caja es de OTRO proyecto. Hasta hoy lo unico que
/// BMO-X sabia de la velocidad de su disco era el catalogo (`450` en el perfil),
/// y cualquier mejora del driver --asincrono, NCQ, PRD multiples-- se iba a
/// juzgar contra nada. Esto da el ANTES.
///
/// SOLO LEE, y el sitio no se elige desde aqui: la particion de datos desde su
/// principio. El numero es cuanto (64 si no se dice, techo 1024).
///
/// El aviso se pinta y se VUELCA antes de llamar, igual que `trim ya`: la
/// llamada tiene el disco para si y no vuelve hasta acabar.
pub(crate) fn banda(dsk: &mut Desktop, p: &bmo::Pantalla, arg: &[u8]) -> After {
    let mib = if arg.is_empty() { Some(0) } else { numero(arg) };
    let Some(mib) = mib else {
        let s = &mut dsk.out.grid;
        s.with_ink(INK_ERR);
        s.text(b"  `");
        s.text(arg);
        s.text(b"` no es un numero de MiB   (disco banda 256)\n");
        s.with_ink(INK_PLAIN);
        paint_status(p, &dsk.run_box, "banda", INK_DIM);
        dsk.field.n = 0;
        return After::Settle;
    };
    section(&mut dsk.out.grid, b"disco: el metro de LECTURA");
    dsk.out.grid.text(b"    leyendo (solo lee; el disco es del metro mientras dura)...\n");
    paint_output(p, &dsk.run_box, &dsk.out.grid);
    p.volcar();

    // Los MB/s de la respuesta son los mismos que salen de `INFO_DISCO_BANDA`:
    // se pinta desde el informe para que la orden y `save` digan UN numero.
    let (motivo, _) = bmo::banda(mib);
    let s = &mut dsk.out.grid;
    if bmo::info(bmo::INFO_DISCO_BANDA) == 0 {
        s.with_ink(INK_ERR);
        s.text(b"    no se midio: ");
        s.text(bmo::banda_en_palabras(motivo));
        s.with_ink(INK_PLAIN);
        s.byte(b'\n');
    } else {
        if motivo != bmo::DISCO_BANDA_HECHA {
            s.with_ink(INK_ERR);
            s.text(b"    A MEDIAS: ");
            s.text(bmo::banda_en_palabras(motivo));
            s.with_ink(INK_PLAIN);
            s.byte(b'\n');
        }
        detalle_banda(s);
    }
    paint_status(p, &dsk.run_box, "banda", INK_DIM);
    dsk.field.n = 0;
    dsk.field.cur = 0;
    After::NextKey
}

/// El detalle de la ultima medida: la cifra, sus ordenes y **contra que techo**.
pub(crate) fn detalle_banda(s: &mut Output) {
    let b = bmo::info(bmo::INFO_DISCO_BANDA);
    let o = bmo::info(bmo::INFO_DISCO_BANDA_ORDEN);
    let us = (b & bmo::DISCO_BANDA_US_MASK).max(1);
    let mib = (b >> bmo::DISCO_BANDA_MIB_SHIFT) & bmo::DISCO_BANDA_MIB_MASK;
    let pct = (b >> bmo::DISCO_BANDA_DATOS_SHIFT) & bmo::DISCO_BANDA_DATOS_MASK;
    let mb_s = mib * 1_048_576 / us;

    campo(s, b"read");
    s.with_ink(INK_GOOD);
    s.dec(mb_s);
    s.text(b" MB/s MEDIDOS");
    s.with_ink(INK_PLAIN);
    s.text(b"   ");
    s.dec(mib);
    s.text(b" MiB en ");
    s.dec(us / 1000);
    s.text(b" ms\n");

    // ** El techo del CABLE, del atomo y no de la caja: el SPD del puerto.
    // SATA codifica 8b/10b, asi que 6 Gb/s son 600 MB/s de datos.
    let hba = bmo::info(bmo::INFO_DISCO_HBA);
    let spd = ((hba >> bmo::DISCO_HBA_SSTS_SHIFT) >> 4) & 0xF;
    if hba & bmo::DISCO_HBA_HAY != 0 && (1..=3).contains(&spd) {
        let techo = 150u64 << (spd - 1);
        campo(s, b"cable");
        s.text(b"Gen");
        s.dec(spd);
        s.text(b" = ");
        s.dec(techo);
        s.text(b" MB/s de techo   se usa el ");
        s.pct(mb_s, techo);
        s.byte(b'\n');
    }

    let sect = o & bmo::DISCO_BANDA_ORDEN_SECTORES_MASK;
    let mejor = (o >> bmo::DISCO_BANDA_ORDEN_MEJOR_SHIFT) & bmo::DISCO_BANDA_ORDEN_US_MASK;
    let peor = (o >> bmo::DISCO_BANDA_ORDEN_PEOR_SHIFT) & bmo::DISCO_BANDA_ORDEN_US_MASK;
    campo(s, b"orders");
    s.dec(sect / 2);
    s.text(b" KiB cada una   la mas rapida ");
    s.dec(mejor);
    s.text(b" us, la mas lenta ");
    s.dec(peor);
    s.text(b" us");
    // Una orden lenta muchas veces la rapida es el disco parandose a medio
    // camino: la media no lo dice, la distancia si.
    if mejor > 0 && peor > mejor * 4 {
        s.with_ink(INK_ERR);
        s.text(b"   SE PARO a medio camino");
        s.with_ink(INK_PLAIN);
    }
    s.byte(b'\n');

    campo(s, b"data");
    if pct < 90 {
        s.with_ink(INK_ERR);
        s.dec(pct);
        s.text(b"% de sectores con datos: el SSD pudo contestar SIN LEER (ceros del mapa)");
        s.with_ink(INK_PLAIN);
    } else {
        s.dec(pct);
        s.text(b"% de sectores con datos: la NAND se leyo de verdad");
    }
    s.byte(b'\n');
    s.with_ink(INK_ECHO);
    s.text(b"    es LECTURA por la ranura 0; la escritura sostenida es otra cifra\n");
    s.with_ink(INK_PLAIN);
}

/// Un decimal, o `None` si no lo es.
fn numero(arg: &[u8]) -> Option<u64> {
    let mut v = 0u64;
    for &c in arg {
        if !c.is_ascii_digit() {
            return None;
        }
        v = v.checked_mul(10)?.checked_add((c - b'0') as u64)?;
    }
    Some(v)
}

/// Una subordem que no existe. Se dice **cual se escribio**, y las que hay.
pub(crate) fn no_existe(dsk: &mut Desktop, p: &bmo::Pantalla, que: &[u8]) -> After {
    let s = &mut dsk.out.grid;
    s.with_ink(INK_ERR);
    s.text(b"  `disco ");
    s.text(que);
    s.text(b"` no es una orden del disco\n");
    s.with_ink(INK_PLAIN);
    ordenes(s);
    paint_status(p, &dsk.run_box, "disco", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}
