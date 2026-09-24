//! **`iommu`: la IOMMU, preguntada.** Si el firmware la dejo encendida, que
//! sabe hacer, a quien atiende y que memoria exige.
//!
//! [consumo] NADA      solo lee: unos `info` de lo que el kernel leyo al arrancar
//!
//! # Por que existe (2026-09-23, M0a de `docs/plan/PLAN_LA_3060.md`)
//!
//! El VBLANK de la 3060 por MSI pide su Bus Master, y el propietario decidio
//! que eso va detras de la IOMMU. Antes de encenderla se pregunta, como se
//! pregunto la grafica -- y la respuesta la da la fila `verdict`.
//!
//! Va tambien en el `save`, capitulo 2, por lo mismo que `gpu`: una pregunta
//! que solo se contesta si alguien teclea la orden es una pregunta que se
//! olvida.

use bmo_userland as bmo;

use super::tabla::{campo, section};
use super::After;
use crate::desktop::Desktop;
use crate::scene::output::{Output, INK_ECHO, INK_ERR, INK_GOOD, INK_PLAIN};
use crate::scene::{paint_status, INK_DIM};
use bmo_iommu_amdvi as amdvi;

/// `iommu`, `iommu encender`, `iommu apagar` desde el escritorio.
///
/// ** `encender` y `apagar` son ORDENES ARRIESGADAS (M0c): escriben en la
/// frontera del DMA de toda la maquina. En modo `save` automatico el informe
/// maestro se guarda ANTES, y si no se puede guardar la orden no se hace. Y el
/// kernel hace `FLUSH CACHE` del disco antes de tocar nada.
pub(crate) fn iommu(dsk: &mut Desktop, p: &bmo::Pantalla, arg: &[u8]) -> After {
    let op = match arg {
        b"" => None,
        b"encender" | b"on" => Some((bmo::IOMMU_OP_ENCENDER, b"iommu encender" as &[u8])),
        b"apagar" | b"off" => Some((bmo::IOMMU_OP_APAGAR, b"iommu apagar" as &[u8])),
        _ => {
            dsk.out.grid.with_ink(INK_ERR);
            dsk.out.grid.text(b"  iommu: `iommu`, `iommu encender` o `iommu apagar`\n");
            dsk.out.grid.with_ink(INK_PLAIN);
            dsk.field.n = 0;
            return After::Settle;
        }
    };
    if let Some((op, nombre)) = op {
        if !super::files::antes_de_arriesgar(dsk, p, nombre) {
            dsk.field.n = 0;
            return After::Settle;
        }
        let g = &mut dsk.out.grid;
        match bmo::iommu_orden(op) {
            Ok(v) if op == bmo::IOMMU_OP_ENCENDER => {
                g.with_ink(INK_GOOD);
                g.text(b"  IOMMU ENCENDIDA y OBEDECE: el COMPLETION_WAIT volvio en ");
                g.dec(v & 0xFFFF_FFFF);
                g.text(b" us; ");
                g.dec(v >> 32);
                g.text(b" eventos en su registro\n");
            }
            Ok(_) => {
                g.with_ink(INK_GOOD);
                g.text(b"  IOMMU APAGADA: el control vuelve a como lo dejo el firmware\n");
            }
            Err(m) => {
                g.with_ink(INK_ERR);
                g.text(b"  NO: ");
                g.text(motivo(m));
                g.byte(b'\n');
            }
        }
        g.with_ink(INK_PLAIN);
    }
    report_iommu(&mut dsk.out.grid);
    paint_status(p, &dsk.run_box, "iommu", INK_DIM);
    dsk.field.n = 0;
    After::Settle
}

/// El motivo de un NO de `TASK_OP_IOMMU`, en palabras.
pub(crate) fn motivo(m: u32) -> &'static [u8] {
    match m {
        bmo::IOMMU_NO_ESCRITORIO => b"solo el escritorio (quien tiene la pantalla) la mueve",
        bmo::IOMMU_NO_TABLAS => b"no hay tablas de M0b releidas iguales (mira la fila `ours`)",
        bmo::IOMMU_NO_YA_ENCENDIDA => b"ya estaba encendida: no se pisa",
        bmo::IOMMU_NO_CONTESTA => b"el COMPLETION_WAIT no volvio en 10 ms: se APAGO sola otra vez",
        bmo::IOMMU_NO_APAGADA => b"la IOMMU no la encendio BMO-X: primero `iommu encender`",
        bmo::IOMMU_NO_SIN_GPU => b"no hay una NVIDIA donde la sonda la vio",
        bmo::IOMMU_NO_GPU_VE => b"EL CANDADO: la 3060 no esta CIEGA en la IOMMU; primero `gpu cegar`",
        bmo::IOMMU_NO_SIN_MSI => b"la 3060 no anuncia MSI",
        bmo::IOMMU_NO_SIN_CABEZA => b"la sonda no dejo cabeza que pinte (mira `gpu`)",
        bmo::IOMMU_NO_SIN_VECTOR => b"el vector 50 no se instalo al arrancar",
        bmo::IOMMU_NO_E2_NO_ARMA => b"la pantalla no acepto el aviso del VBLANK: se deshizo y el Bus Master se retiro",
        bmo::IOMMU_NO_SIN_AREA => b"no hubo paginas contiguas para las tablas del dominio de la 3060",
        bmo::IOMMU_NO_NO_TRADUCIDA => b"prestar pide la 3060 TRADUCIDA: primero `gpu traducir`",
        bmo::IOMMU_NO_PRESTAMO => b"el prestamo no se hizo (ya prestado, o sin tablas): mira `cabina fallos`",
        bmo::IOMMU_NO_RELEIDA => b"el ORACULO no vio lo prestado al releer las tablas: se quito",
        bmo::IOMMU_NO_SIN_BUS_MASTER => b"sin el Bus Master de E2 la 3060 no hace DMA: primero `gpu vblank`",
        bmo::IOMMU_NO_FUEGO => b"el falcon del GSP no dejo hacer el DMA: el motivo, en la fila `fuego` de `gpu`",
        bmo::IOMMU_NO_SIN_PRUEBA => b"la pagina de prueba no esta prestada: primero `gpu prestar`",
        super::gpu::NO_FUEGO_A_MEDIAS => b"el DMA acabo pero la DMEM NO es la pagina prestada: mira la fila `fuego`",
        super::gpu::NO_SIN_FRONTERA => b"la IOMMU NO apunto el fallo de pagina de la 3060: mira la fila `frontera`",
        super::vbios::NO_SIN_MEMORIA => b"no hubo 1 MiB para leer la ROM",
        super::vbios::NO_SIN_FWSEC => b"la VBIOS se leyo pero FWSEC no se entendio: mira la fila `fwsec` de `gpu`",
        super::vbios::NO_SIN_FIRMA => b"FWSEC no trae firma para el fusible de esta tarjeta: mira la fila `fusible`",
        bmo::IOMMU_NO_FWSEC_DESC => b"el descriptor de la ROM no es un FWSEC v3 del GSP que quepa",
        bmo::IOMMU_NO_FWSEC_SIN_PREPARAR => b"FWSEC sin preparar, o faltan trozos de la ROM",
        bmo::IOMMU_NO_FWSEC_FIRMA => b"el fusible no pide ninguna de las firmas de FWSEC",
        bmo::IOMMU_NO_FWSEC_PARCHE => b"la orden FRTS o la firma no se pudieron poner en el ucode",
        bmo::IOMMU_NO_WPR2_YA => b"YA hay WPR2: la 3060 necesita un reinicio para volver a correr FWSEC",
        bmo::IOMMU_NO_GFW => b"el firmware de arranque de la tarjeta no acabo (o no se deja leer)",
        bmo::IOMMU_NO_FWSEC_FALCON => b"el falcon del GSP no dejo resetearse o cargar FWSEC por DMA",
        super::vbios::NO_SIN_VBIOS => b"la VBIOS no se leyo o no trae FWSEC: `gpu vbios`",
        super::vbios::NO_FWSEC_NO_PARA => b"FWSEC ARRANCO y no se paro en 3 s: mira la fila `frts`",
        super::vbios::NO_FWSEC_MAL => b"FWSEC se paro pero no dejo la WPR2: MAILBOX0 o el codigo FRTS en la fila `frts`",
        bmo::IOMMU_NO_GSP_FICHERO => b"no esta fw/gsp/gsp.bin o fw/gsp/bootldr.bin en el disco de datos (el build los baja)",
        bmo::IOMMU_NO_GSP_FORMATO => b"fw/gsp/ no trae un GSP-RM con .fwimage y la firma ga10x, o un bootloader que quepa",
        bmo::IOMMU_NO_GSP_MARCOS => b"no hubo marcos para los 61 MB del GSP-RM, su radix3 o lo auxiliar",
        bmo::IOMMU_NO_GSP_ORDEN => b"fuera de orden: sin preparar, un trozo saltado o prestar sin copiar todo",
        bmo::IOMMU_NO_GSP_DISCO => b"el disco devolvio menos del GSP-RM de lo pedido",
        bmo::IOMMU_NO_GSP_YA_PRESTADO => b"el GSP-RM ya esta prestado: no se escribe encima de lo que la 3060 puede leer",
        bmo::IOMMU_NO_GSP_SIN_VRAM => b"la VRAM no se deja repartir para el GSP (mira la fila `mapa`)",
        bmo::IOMMU_NO_GSP_RADIX => b"la radix3 NO lleva, por la IOMMU, a la pagina que tiene que llevar",
        bmo::IOMMU_NO_LIBOS_ORDEN => b"primero el GSP-RM prestado por su radix3 (`gpu radix`)",
        bmo::IOMMU_NO_LIBOS_MARCOS => b"no hubo marcos para los argumentos de LIBOS, los logs o las colas",
        bmo::IOMMU_NO_LIBOS_PUNTERO => b"un puntero de LIBOS NO lleva, por la IOMMU, a donde tiene que llevar",
        bmo::IOMMU_NO_DESPERTAR_ANTES => b"falta algo de ESTE arranque: la WPR2 de FWSEC, `gpu radix` o `gpu libos`",
        bmo::IOMMU_NO_BOOTER => b"fw/gsp/boot_ld.bin no esta, o no es un booter del SEC2 que quepa",
        bmo::IOMMU_NO_BOOTER_FIRMA => b"el fusible del SEC2 no pide ninguna firma del booter",
        bmo::IOMMU_NO_VACIADO => b"la pagina de vaciado no se releyo en 0x100C10",
        bmo::IOMMU_NO_GSP_FALCON => b"el falcon del GSP no se reseteo, o no estaba parado para el booter",
        bmo::IOMMU_NO_SEC2 => b"el SEC2 no dejo resetearse, cargar el booter por DMA o arrancar",
        bmo::IOMMU_NO_SEC2_NO_PARA => b"el SEC2 todavia no se paro",
        bmo::IOMMU_NO_BOOTER_MAL => b"el booter se paro con un ERROR en MAILBOX0: mira la fila `despierto`",
        bmo::IOMMU_NO_COLA_ANTES => b"el GSP no desperto en este arranque: no hay cola que devolverle",
        bmo::IOMMU_NO_COLA_PUNTERO => b"el puntero de lectura pedido no es un hueco de la cola (0..62)",
        bmo::IOMMU_NO_SISTEMA_ANTES => b"sin `gpu libos` no hay cola de la CPU donde escribirle al GSP",
        bmo::IOMMU_NO_SISTEMA_YA => b"SetSystemInfo y SetRegistry ya se mandaron, o el GSP ya desperto: van antes y una vez",
        bmo::IOMMU_NO_RPC_ANTES => b"el GSP-RM no esta arrancado: primero `gpu init` hasta GSP_INIT_DONE",
        bmo::IOMMU_NO_RPC_OBJETO => b"no es uno de nuestros tres objetos del RM (cliente, dispositivo, subdispositivo)",
        bmo::IOMMU_NO_RPC_LLENA => b"la cola de la CPU esta llena: el GSP-RM no ha leido lo de antes",
        bmo::IOMMU_NO_BAR1 => b"no hay BAR1 que devolver: primero `gpu init` hasta GSP_INIT_DONE",
        bmo::IOMMU_NO_SEC_ANTES => b"el GSP no desperto, o lo primero de su cola no es un secuenciador entero",
        bmo::IOMMU_NO_SEC_FALLO => b"el secuenciador se paro en una orden: mira las filas `corrio` y `orden`",
        bmo::IOMMU_NO_SEC_YA => b"el secuenciador ya se corrio en este arranque: otra vez pide apagar",
        bmo::IOMMU_NO_YA_DESPIERTO => b"el booter ya corrio en este arranque: otra vez pide reiniciar la maquina",
        super::gsp::NO_GSP_NO_PARA => b"el GSP arranco con sus argumentos y no se paro en 2 s",
        super::gsp::NO_SEC2_NO_ACABA => b"el booter ARRANCO en el SEC2 y no se paro en 5 s",
        super::gsp::NO_RISCV_DORMIDO => b"el booter acabo bien pero el RISC-V del GSP no se encendio en 5 s: mira datos/gsplog.bin",
        super::gspcola::NO_COLA_SIN_GSP => b"el GSP no ha despertado en este arranque: `gpu despertar`",
        super::gspvaciar::NO_VACIAR_MAL => b"al vaciar, un mensaje sin forma o con la suma mal: se paro ahi sin consumirlo (mira la fila `pide`)",
        bmo::IOMMU_NO_TRAMO => b"la entrada de la raiz para el tramo ya estaba ocupada (no se pisa), o el tramo ya se mapeo",
        super::gspvram::NO_TRAMO_MAL => b"las tablas se escribieron pero alguna entrada no se releyo igual",
        bmo::IOMMU_NO_DIRECTORIO_YA => b"el directorio ya se puso en este arranque (el RM ya escribio en el): no se repite",
        super::gspvram::NO_DIRECTORIO_NEGADO => b"el GSP-RM contesto pero NO acepto el directorio: su NV_STATUS, en la fila `pd`",
        bmo::IOMMU_NO_VRAM => b"sin 3060 que probar, o la prueba de la VRAM ya estaba en curso",
        super::gspvram::NO_VRAM_SIN_REGIONES => b"sin las regiones de VRAM del GSP-RM: primero `gpu estatica`",
        super::gspvram::NO_VRAM_NO_USABLE => b"la pagina de prueba NO cae en VRAM usable segun el GSP-RM: no se toca",
        super::gspvram::NO_VRAM_MAL => b"la prueba corrio y algo no cuadro (patron, lo devuelto o la ventana)",
        super::gspmotores::NO_MOTORES_SIN_COPIA => b"el GSP-RM dio la lista de motores y no trae COPY2, el de copia del canal: mira la fila `motores`",
        bmo::IOMMU_NO_CANAL => b"sin el tramo mapeado (`gpu tramo`), o el canal ya se pidio en este arranque",
        bmo::IOMMU_NO_CANAL_MEMORIA => b"las paginas del canal no quedaron a cero en la VRAM, o su bufer de metodos no se presto a la 3060",
        bmo::IOMMU_NO_CANAL_ORDEN => b"una orden del canal fuera de su sitio: el canal no se pidio todavia",
        super::gspcanal::NO_CANAL_SIN_MOTORES => b"sin la lista de motores con COPY2: primero `gpu motores`",
        super::gspcanal::NO_CANAL_METODOS => b"el RM dice que el bufer de metodos mide otra cosa que los 20480 B que se prestan: el canal NO se pide",
        super::gspcanal::NO_CANAL_NEGADO => b"el GSP-RM contesto pero NO dio el canal: su NV_STATUS, en la fila `canal`",
        super::gspsalud::NO_CONTROL_NEGADO => b"el GSP-RM contesto pero la orden de control no salio: su NV_STATUS, en la fila `pstate` (o `motores`/`metodos`)",
        bmo::IOMMU_NO_RPC_CONTRATO => b"el mensaje no esta en el CONTRATO de la cola (bmo_gpu_ga10x::contrato): no salio",
        bmo::IOMMU_NO_RPC_CONTROL => b"no es una de las ordenes de control de la lista",
        super::gspobjeto::NO_OBJ_NEGADO => b"el GSP-RM contesto pero NO dio el objeto: su NV_STATUS, en la fila `obj`",
        super::gsprpc::NO_RPC_SIN_RESPUESTA => b"el GSP-RM no contesto la RPC en 5 s (o llego un mensaje sin forma)",
        super::gspinit::NO_INIT_NO_LLEGA => b"el GSP-RM no dijo GSP_INIT_DONE a tiempo: mira las filas `corrio` y `listo`",
        super::gspsecuencia::NO_SEC_NO_HAY => b"lo primero de la cola del GSP no es un GSP_RUN_CPU_SEQUENCER: mira las filas `cola` y `pide`",
        super::gspsecuencia::NO_SEC_MAL => b"el secuenciador no suma 0 o trae una orden que no se entiende: mira las filas `orden` y datos/gspsec.bin",
        super::gspcola::NO_COLA_MAL => b"la cola del GSP trae un mensaje sin forma o con la suma mal: mira la fila `cola` y datos/gspcola.bin",
        super::gsp::NO_RADIX_NO_CUADRA => b"prestado, pero lo visto por la radix3 no es lo copiado o no es la 570.144: mira la fila `radix`",
        super::gsp::NO_GSP_INCOMPLETO => b"el firmware del GSP no esta entero o no cuadra: mira las filas `booter` a `cuadra` de `gpu`",
        super::gpu::NO_E2_MUDO => b"E2 quedo ARMADO pero no llego ni un VBLANK: mira la escalera de `gpu`",
        _ => b"el kernel dijo que no, sin motivo conocido",
    }
}

/// Un BDF como `bb:dd.f`.
fn bdf(s: &mut Output, v: u64) {
    s.hex((v >> 8) & 0xFF, 2);
    s.byte(b':');
    s.hex((v >> 3) & 0x1F, 2);
    s.byte(b'.');
    s.dec(v & 7);
}

/// Una bandera con nombre, si esta.
fn si(s: &mut Output, esta: bool, nombre: &[u8]) {
    if esta {
        s.byte(b' ');
        s.text(nombre);
    }
}

/// **El cuadro de la IOMMU.** Lo usan `iommu` y el `save`.
pub(crate) fn report_iommu(s: &mut Output) {
    section(s, b"iommu -- la frontera del DMA de todo aparato");
    let d = bmo::info(bmo::INFO_IOMMU_DONDE);
    campo(s, b"where");
    if d & bmo::IOMMU_HALLADA == 0 {
        s.with_ink(INK_ERR);
        s.text(b"el IVRS no trae una IOMMU que se lea: nada limita el DMA\n");
        s.with_ink(INK_PLAIN);
        return;
    }
    let base = (d & bmo::IOMMU_BASE_PAGINAS_MASK) << 12;
    s.text(b"IVHD tipo 0x");
    s.hex((d >> bmo::IOMMU_TIPO_SHIFT) & 0xFF, 2);
    s.text(b"   registros en 0x");
    s.hex(base, 8);
    s.with_ink(INK_ECHO);
    s.text(b"   su BDF ");
    bdf(s, (d >> bmo::IOMMU_BDF_SHIFT) & 0xFFFF);
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');
    super::datos::anotar(b"iommu base", base, b"");

    let control = bmo::info(bmo::INFO_IOMMU_CONTROL);
    let funciones = bmo::info(bmo::INFO_IOMMU_FUNCIONES);
    if d & bmo::IOMMU_MUDA != 0 {
        veredicto(s, false, b"sus registros no contestan (todo unos, o fuera del physmap)");
        return;
    }

    let c = amdvi::Control(control);
    let viva = bmo::info(bmo::INFO_IOMMU_VIVA);
    campo(s, b"state");
    if c.encendida() && viva & bmo::IOMMU_VIVA_ENCENDIDA != 0 {
        s.with_ink(INK_GOOD);
        s.text(b"ENCENDIDA por BMO-X (M0c): todo DE PASO por ahora");
    } else if c.encendida() {
        s.with_ink(INK_ERR);
        s.text(b"ENCENDIDA por el firmware");
    } else {
        s.with_ink(INK_GOOD);
        s.text(b"APAGADA: nadie traduce, todo aparato ve toda la RAM");
    }
    s.with_ink(INK_ECHO);
    s.text(b"   control 0x");
    s.hex(control, 16);
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');
    super::datos::anotar(b"iommu control", control, b"");

    let f = amdvi::Funciones(funciones);
    campo(s, b"can");
    s.text(b"paginas de hasta ");
    s.dec(f.niveles() as u64);
    s.text(b" niveles;");
    si(s, f.nx(), b"NX");
    si(s, f.x2apic(), b"x2APIC");
    si(s, f.ga(), b"GA");
    si(s, f.invalidar_todo(), b"INVALIDAR-TODO");
    si(s, f.gt(), b"GT");
    si(s, f.ppr(), b"PPR");
    si(s, f.prefetch(), b"PREFETCH");
    si(s, f.he(), b"HE");
    si(s, f.contadores(), b"CONTADORES");
    s.with_ink(INK_ECHO);
    s.text(b"   EFR 0x");
    s.hex(funciones, 16);
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');
    super::datos::anotar(b"iommu efr", funciones, b"");

    let t = bmo::info(bmo::INFO_IOMMU_TABLA);
    let e = amdvi::Estado(bmo::info(bmo::INFO_IOMMU_ESTADO));
    campo(s, b"tables");
    match amdvi::Tabla::de_registro(t) {
        Some(tb) => {
            s.text(b"ya hay tabla de dispositivos en 0x");
            s.hex(tb.base, 8);
            s.text(b" (");
            s.dec(tb.entradas());
            s.text(b" BDF)");
        }
        None => s.text(b"ninguna armada"),
    }
    s.text(if e.ordenes_corren() { b"; ordenes CORREN" as &[u8] } else { b"; ordenes paradas" });
    s.text(if e.eventos_corren() { b"; eventos CORREN" as &[u8] } else { b"; eventos parados" });
    s.byte(b'\n');
    fila_armado(s, viva & bmo::IOMMU_VIVA_ENCENDIDA != 0);
    fila_viva(s, viva);
    fila_gpu(s);
    fila_evento(s);

    let n = bmo::info(bmo::INFO_IOMMU_CENSO);
    if n & bmo::IOMMU_CENSO_VALIDO != 0 {
        let max = n & 0xFFFF;
        let campo8 = |sh: u64| (n >> sh) & 0xFF;
        campo(s, b"serves");
        s.dec(campo8(bmo::IOMMU_CENSO_UNOS_SHIFT));
        s.text(b" sueltos, ");
        s.dec(campo8(bmo::IOMMU_CENSO_RANGOS_SHIFT));
        s.text(b" rangos, ");
        s.dec(campo8(bmo::IOMMU_CENSO_ALIAS_SHIFT));
        s.text(b" alias");
        if n & bmo::IOMMU_CENSO_TODOS != 0 {
            s.text(b", TODOS los BDF");
        }
        s.text(b"; el mayor ");
        bdf(s, max);
        s.with_ink(INK_ECHO);
        s.text(b" -> tabla de ");
        s.dec(amdvi::tabla_para(max as u16) / 1024);
        s.text(b" KiB");
        s.with_ink(INK_PLAIN);
        let raras = (n >> bmo::IOMMU_CENSO_RARAS_SHIFT) & 0xF;
        if raras > 0 {
            s.text(b"; ");
            s.dec(raras);
            s.text(b" entradas por HID o sin nombre");
        }
        if n & bmo::IOMMU_CENSO_CORTADO != 0 {
            s.with_ink(INK_ERR);
            s.text(b"  CORTADO: una entrada se salia del bloque");
            s.with_ink(INK_PLAIN);
        }
        s.byte(b'\n');
        super::datos::anotar(b"iommu mayor bdf", max, b"");

        for i in 0..campo8(bmo::IOMMU_CENSO_ESPECIALES_SHIFT).min(8) {
            let x = bmo::info(bmo::INFO_IOMMU_ESPECIAL | i << bmo::IOMMU_INDICE_SHIFT);
            if x & bmo::IOMMU_VALIDA == 0 {
                continue;
            }
            campo(s, b"special");
            s.text(match (x >> 24) & 0xFF {
                1 => b"IOAPIC id 0x" as &[u8],
                2 => b"HPET   n.  0x",
                _ => b"??     0x",
            });
            s.hex((x >> 16) & 0xFF, 2);
            s.text(b" pide como ");
            bdf(s, x & 0xFFFF);
            s.with_ink(INK_ECHO);
            s.text(b"   banderas 0x");
            s.hex((x >> 32) & 0xFF, 2);
            s.with_ink(INK_PLAIN);
            s.byte(b'\n');
        }

        let nm = campo8(bmo::IOMMU_CENSO_IVMD_SHIFT).min(8);
        if nm == 0 {
            campo(s, b"ivmd");
            s.text(b"ninguno: el firmware no exige memoria por DMA\n");
        }
        for i in 0..nm {
            let sel = bmo::INFO_IOMMU_IVMD | i << bmo::IOMMU_INDICE_SHIFT;
            let meta = bmo::info(sel | 2 << bmo::IOMMU_PARTE_SHIFT);
            if meta & bmo::IOMMU_VALIDA == 0 {
                continue;
            }
            let inicio = bmo::info(sel);
            let largo = bmo::info(sel | 1 << bmo::IOMMU_PARTE_SHIFT);
            let ban = (meta >> 40) & 0xFF;
            campo(s, b"ivmd");
            s.text(b"0x");
            s.hex(inicio, 8);
            s.text(b" + ");
            s.dec(largo / 1024);
            s.text(b" KiB");
            si(s, ban & 0x01 != 0, b"IDENTIDAD");
            si(s, ban & 0x02 != 0, b"lee");
            si(s, ban & 0x04 != 0, b"escribe");
            si(s, ban & 0x08 != 0, b"EXCLUSION");
            s.text(match (meta >> 32) & 0xFF {
                0x20 => b"   para TODOS" as &[u8],
                0x21 => b"   para ",
                _ => b"   para el rango ",
            });
            if (meta >> 32) & 0xFF != 0x20 {
                bdf(s, meta & 0xFFFF);
                if (meta >> 32) & 0xFF == 0x22 {
                    s.text(b"..");
                    bdf(s, (meta >> 16) & 0xFFFF);
                }
            }
            s.byte(b'\n');
        }
    }

    // ** Encendida por BMO-X, el veredicto de la sonda (que mira el control
    // como si fuera la foto del arranque) diria "la dejo el firmware". No.
    if viva & bmo::IOMMU_VIVA_ENCENDIDA != 0 && c.encendida() {
        veredicto(s, true, b"ENCENDIDA POR BMO-X y obedece (M0c), todo de paso; lo siguiente es TRADUCIR (M0d)");
        return;
    }
    match amdvi::veredicto(control, funciones) {
        amdvi::Veredicto::SePuede => veredicto(s, true, b"apagada y contesta: se puede encender con tablas de BMO-X"),
        amdvi::Veredicto::EncendidaPorElFirmware => {
            veredicto(s, false, b"el firmware la dejo TRADUCIENDO: hay que heredar sus tablas, no pisarlas")
        }
        amdvi::Veredicto::Muda => veredicto(s, false, b"sus registros no contestan"),
    }
}

fn veredicto(s: &mut Output, si: bool, frase: &[u8]) {
    campo(s, b"verdict");
    s.with_ink(if si { INK_GOOD } else { INK_ERR });
    s.text(if si { b"SE PUEDE: " as &[u8] } else { b"NO ASI: " });
    s.text(frase);
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');
    super::datos::anotar(b"iommu se puede encender", si as u64, b"");
}

/// ** M0b: las tablas de BMO-X, armadas en RAM y SIN ENTREGAR. Si la fila
/// dice `releida igual`, M0c tiene lo que darle a la IOMMU.
fn fila_armado(s: &mut Output, entregada: bool) {
    let a = bmo::info(bmo::INFO_IOMMU_ARMADO);
    let c = bmo::info(bmo::INFO_IOMMU_COLAS);
    campo(s, b"ours");
    if a & bmo::IOMMU_ARMADO_SI == 0 {
        s.with_ink(INK_ECHO);
        s.text(b"sin armar (no se pudo encender, o no hubo paginas contiguas: `cabina fallos`)\n");
        s.with_ink(INK_PLAIN);
        return;
    }
    let paginas = (a >> bmo::IOMMU_ARMADO_PAGINAS_SHIFT) & 0xFFF;
    s.text(b"tabla de ");
    s.dec(paginas * 4);
    s.text(b" KiB en 0x");
    s.hex((a & bmo::IOMMU_BASE_PAGINAS_MASK) << 12, 8);
    s.text(b", todo DE PASO; ");
    s.dec((a >> bmo::IOMMU_ARMADO_BANDERAS_SHIFT) & 0x3FFF);
    s.text(b" con banderas del IVHD; colas de ");
    s.dec((c >> 36) & 0xFFFF);
    s.text(b" en 0x");
    s.hex((c & bmo::IOMMU_BASE_PAGINAS_MASK) << 12, 8);
    if a & bmo::IOMMU_ARMADO_COMPROBADO != 0 {
        s.with_ink(INK_GOOD);
        s.text(if entregada {
            b"   releida igual, ENTREGADA: es la que la IOMMU usa\n" as &[u8]
        } else {
            b"   releida igual, SIN ENTREGAR\n"
        });
    } else {
        s.with_ink(INK_ERR);
        s.text(b"   releida DISTINTA de lo escrito\n");
    }
    s.with_ink(INK_PLAIN);
    super::datos::anotar(b"iommu tabla armada", a, b"");
}

/// ** M0c: lo que paso al encenderla. Solo sale si alguien lo intento.
fn fila_viva(s: &mut Output, v: u64) {
    let intentos = (v >> bmo::IOMMU_VIVA_INTENTOS_SHIFT) & 0xF;
    if intentos == 0 {
        return;
    }
    campo(s, b"live");
    if v & bmo::IOMMU_VIVA_ENCENDIDA != 0 {
        s.with_ink(INK_GOOD);
        s.text(b"ENCENDIDA; el COMPLETION_WAIT volvio en ");
        s.dec(v & 0xFFFF_FFFF);
        s.text(b" us; eventos ");
        s.dec((v >> bmo::IOMMU_VIVA_EVENTOS_SHIFT) & 0xFFFF);
    } else if v & bmo::IOMMU_VIVA_CONTESTO != 0 {
        s.with_ink(INK_ECHO);
        s.text(b"se encendio y contesto; ahora APAGADA");
    } else {
        s.with_ink(INK_ERR);
        s.text(b"no se pudo: ");
        s.text(motivo(((v >> bmo::IOMMU_VIVA_MOTIVO_SHIFT) & 0xFF) as u32));
    }
    s.with_ink(INK_ECHO);
    s.text(b"   (");
    s.dec(intentos);
    s.text(b" intento(s))\n");
    s.with_ink(INK_PLAIN);
    super::datos::anotar(b"iommu viva", v, b"");
}

/// ** M0e: la 3060, ciega o no. Solo sale si alguien la cego alguna vez.
fn fila_gpu(s: &mut Output) {
    let g = bmo::info(bmo::INFO_IOMMU_GPU);
    if g == 0 {
        return;
    }
    campo(s, b"3060");
    if g & bmo::IOMMU_GPU_CIEGA != 0 {
        s.with_ink(INK_GOOD);
        s.text(b"CIEGA: su DMA no alcanza la RAM; sus interrupciones si pasan");
    } else if g & bmo::IOMMU_GPU_TRADUCIDA != 0 {
        s.with_ink(INK_GOOD);
        s.text(b"TRADUCIDA (M0d): ve SOLO lo que su dominio presta; sus interrupciones pasan");
    } else {
        s.with_ink(INK_ECHO);
        s.text(b"VE: su entrada esta DE PASO otra vez");
    }
    s.with_ink(INK_ECHO);
    s.text(b"   BDF ");
    bdf(s, g & 0xFFFF);
    s.text(b"; la invalidacion volvio en ");
    s.dec((g >> bmo::IOMMU_GPU_US_SHIFT) & 0xFFFF_FFFF);
    s.text(b" us");
    if g & bmo::IOMMU_GPU_RELEIDA == 0 {
        s.with_ink(INK_ERR);
        s.text(b"; la entrada releida NO dice lo escrito");
    }
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');
    super::datos::anotar(b"iommu gpu", g, b"");
    fila_dominio(s);
}

/// ** M0d: el dominio de la 3060 y lo que tiene prestado.
fn fila_dominio(s: &mut Output) {
    let d = bmo::info(bmo::INFO_IOMMU_DOMINIO);
    if d & bmo::IOMMU_DOMINIO_ARMADO == 0 {
        return;
    }
    campo(s, b"domain");
    s.text(b"el de la 3060: ");
    s.dec((d >> bmo::IOMMU_DOMINIO_PRESTADAS_SHIFT) & 0xFF_FFFF);
    s.text(b" pagina(s) prestada(s)");
    s.with_ink(INK_ECHO);
    s.text(b"   tablas ");
    s.dec(d & 0xFFFF);
    s.text(b" de ");
    s.dec((d >> bmo::IOMMU_DOMINIO_AREA_SHIFT) & 0xFFFF);
    let p = bmo::info(bmo::INFO_GPU_PRUEBA);
    if p & bmo::GPU_PRUEBA_PRESTADA != 0 {
        s.text(b"; prueba en 0x10000000 -> 0x");
        s.hex(p & !bmo::GPU_PRUEBA_PRESTADA, 8);
        s.text(b" (solo lectura)");
    }
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');
    super::datos::anotar(b"iommu dominio", d, b"");
}

/// ** M0d: el ULTIMO evento de la IOMMU, con su nombre, su BDF y su direccion.
/// Un aparato que toca lo que no se le presto sale aqui, no en la RAM.
pub(crate) fn fila_evento(s: &mut Output) {
    let e = bmo::info(bmo::INFO_IOMMU_EVENTO);
    if e & bmo::IOMMU_EVENTO_HAY == 0 {
        return;
    }
    let ev = amdvi::tablas::Evento([
        ((e >> bmo::IOMMU_EVENTO_BDF_SHIFT) & 0xFFFF) as u32,
        (((e >> bmo::IOMMU_EVENTO_TIPO_SHIFT) & 0xF) << 28 | ((e >> bmo::IOMMU_EVENTO_BANDERAS_SHIFT) & 0xFFF) << 16) as u32,
        0,
        0,
    ]);
    campo(s, b"event");
    s.with_ink(INK_ERR);
    s.dec(e & 0xFFFF);
    s.text(b" pendiente(s); el ultimo: ");
    s.text(ev.nombre().as_bytes());
    s.with_ink(INK_ECHO);
    s.text(b"   BDF ");
    bdf(s, ev.bdf() as u64);
    s.text(b" direccion 0x");
    s.hex(bmo::info(bmo::INFO_IOMMU_EVENTO_DIR), 16);
    s.text(b" banderas 0x");
    s.hex(ev.banderas() as u64, 3);
    s.with_ink(INK_PLAIN);
    s.byte(b'\n');
    super::datos::anotar(b"iommu evento", e, b"");
}
