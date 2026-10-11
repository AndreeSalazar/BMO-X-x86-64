//! **Despertar los otros nucleos.** El bring-up de SMP, del lado bueno de la
//!
//! [carril]  ROJO      despertar los otros nucleos
//! [consumo] APARATO   levanta los otros nucleos y los para; con ellos en pie,
//!                     cada obrero late (ver crew.rs)
//! frontera.
//!
//! === Por que esto esta aqui y no en `s1_cpu` ===
//!
//! Habia un `smp_startup()` en `faggin/s1_cpu` que **nunca se llamo**, y al ir a
//! llamarlo se vio que estaba **del lado equivocado de `ExitBootServices`**:
//! antes de EBS el firmware sigue vivo y **los otros nucleos son suyos** (UEFI
//! los tiene en su *MP Services*), la memoria baja tampoco es nuestra, y lo
//! siguiente que hacia `s1_cpu` era volver a llamar al firmware. Encima hablaba
//! solo por `ser_print!`, y en esta maquina no hay cable serie.
//!
//! Aqui, en el kernel, no hay firmware: la maquina es entera de BMO, la memoria
//! baja **ya esta reservada** (`phys::init` reserva `<1 MiB` con el comentario
//! *"future SMP trampoline lives here"*) y CABINA pinta en pantalla.
//!
//! === Las cuatro piezas, y por que estan separadas ===
//!
//! | | |
//! |---|---|
//! | [`mapa`] | las direcciones y la GDT: **el dato** que los dos lados comparten |
//! | [`tramp`] | el trampolin de 16->64 bits y donde aterriza el AP |
//! | [`lapic`] | mandar IPIs y esperar -- solo lo usa el que despierta |
//! | este fichero | **la orquestacion, y nada mas** |
//!
//! No es orden por el orden: `mapa` existe porque esas direcciones estan
//! escritas **dos veces** --aqui en Rust y a mano dentro del trampolin, que no
//! puede llamar a nada--, y tenerlas en un fichero con nombre propio es lo que
//! hace que se vean juntas. Y `lapic` esta aparte porque es justo lo que el AP
//! **no** puede tocar.
//!
//! === PROBADO EN METAL: `12 de 12` el 2026-08-07 ===
//!
//! Y **sigue sin llamarse en el arranque**, que ya no es cautela sino la
//! decision: se pide con la orden `smp`. INIT+SIPI es la unica operacion del
//! sistema que no se deshace sin reiniciar, asi que si algo va mal se cuelga un
//! comando y no la maquina al encenderla.

pub mod lapic;
pub mod map;
/// El reparto de trabajo. Sin esto, los nucleos despiertos no sirven de nada.
pub mod banda;
/// LA PRUEBA DE LA RAM (`ram prueba`): patrones en la RAM libre, releidos.
pub mod prueba_ram;
/// LA FICHA DE CADA OBRERO: quien es --nucleo y hilo-- y que lleva hecho
/// ahora mismo. Es lo que separa "doce partes iguales" de un reparto que
/// sabe que dos hilos comparten un nucleo.
pub mod ficha;
pub mod crew;
/// El bucle del obrero: lo que LATE mientras los nucleos estan en pie (L6h).
pub mod obrero;

// ** LO QUE LE FALTABA A AXION PARA PODER ENCENDER. `crew` dejo escrito el
// precio --once nucleos girando al 100%-- antes de que existiera la salida;
// esto es la salida, y solo se usa si el silicio la trae con PLAZO.
pub mod dormir;
/// **EL ATRIL**: donde Ring 3 deja el encargo antes de decir *tocad*. `crew`
/// sabia repartir desde el 08-08 y nadie le habia dado nunca una faena de
/// verdad; esto es lo que faltaba, y no era el reparto: era **como se le dicen
/// los numeros**. Ver la cabecera del modulo.
pub mod atril;
pub mod tramp;
/// La GDT y el TSS de cada obrero: lo que le falta a un AP para poder tomar
/// una excepcion sin reiniciar la maquina (A1 de PLAN_EL_BUS_APARTE).
pub mod tss;
// ** LOS SUB-DIRECTORES (H4.3, 07-10): una parte de la faena de una app, en
// Ring 3, en cada obrero. Todo lo que pisa el CPU para eso vive alli.
pub mod ring3;
// ** LA FILA DE ESTA CPU (H4.0, 07-10): hermanos SMT, L3 y nucleos grandes o
// chicos, para repartir primero un nucleo fisico por parte.
pub mod topologia;

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use map::*;

/// El ultimo `despertar` NO desperto a nadie porque ya habia obreros en pie
/// (11-10): la caja lo dice en vez de pintar un `12 de 12` en verde.
static REHUSADO: AtomicBool = AtomicBool::new(false);
/// El CR0/CR4 de los obreros contra el BSP (`INFO_SMP_CPU`): bit 63 medido,
/// obreros `[0..8)`, distintos `[8..16)`, sin cache `[16..24)`, sin MCE
/// `[24..32)`.
static VEREDICTO: AtomicU64 = AtomicU64::new(0);

/// Si el ultimo `despertar` se nego a despertar otra vez.
pub fn rehusado() -> bool {
    REHUSADO.load(Ordering::SeqCst)
}

/// El veredicto del CR0/CR4 de los obreros (`INFO_SMP_CPU`).
pub fn veredicto_cpu() -> u64 {
    VEREDICTO.load(Ordering::SeqCst)
}

/// Despierta a los demas nucleos y cuenta cuantos contestan.
///
/// Devuelve `(alive, esperados)`, ambos **sin contar el BSP**.
///
/// * `aviso` se llama **antes de cada SIPI**, con el APIC ID al que le toca. No
/// es adorno: despertar a un AP cuesta hasta 10 ms de espera y esto es lo unico
/// que puede colgarse de todo el comando. Sin el aviso, un cuelgue se ve como
/// *"se quedo parado"*; con el se ve **en que nucleo** se quedo parado, que es
/// la diferencia entre tener un dato y tener que adivinar.
///
/// Va como parametro y no como una llamada a CABINA desde aqui por dos razones:
/// once lineas seguidas inundarian un anillo de 48 eventos, y **`plat/` no tiene
/// por que saber pintar**. Quien llama decide como se muestra.
///
/// * `cuantos` es el CONTROL, y es lo que el propietario pidio: **0 no despierta a
/// nadie** y solo contesta el censo, `u32::MAX` despierta a todos, y cualquier
/// otro numero despierta exactamente esos, en el orden en que los lista el
/// firmware.
///
/// Que el caso por defecto sea *"mira y no toques"* no es prudencia: mandar
/// INIT+SIPI es la unica operacion de todo el sistema que cambia el estado del
/// hardware de forma que no se puede deshacer sin reiniciar. Un mando asi se
/// dispara **a proposito**, no por escribir su nombre.
pub fn despertar(cuantos: u32, aviso: impl Fn(u32)) -> (u32, u32) {
    // -- A quien llamar: lo dice el FIRMWARE, no una suposicion ----------
    //
    // La MADT es la lista de nucleos que declara la placa. Antes esto suponia
    // APIC IDs `0..hilos-1`, cierto en un Zen 3 de un CCD y falso en cuanto se
    // cambie de maquina -- y fallando de la peor forma, porque un ID inventado y
    // un nucleo no llamado se ven **igual desde fuera**.
    let censo = super::madt::censo();

    // Por el PERFIL, no por el nombre del fabricante. Ver `cpu_vendor/profile.rs`.
    let por_cpuid = (crate::ring0::cpu_vendor::profile::active().nucleos)().map(|n| n.hilos);

    // * Y se contrastan las dos fuentes. Dicen cosas distintas por naturaleza:
    // CPUID dice lo que el silicio TIENE, la MADT lo que el firmware DECLARA. Si
    // no coinciden, lo normal es que la placa haya apagado algo (SMT off en la
    // BIOS) -- y saberlo aqui evita buscar un fallo en el trampolin cuando lo que
    // pasa es que faltan nucleos a proposito.
    if let (Some(c), Some(hilos)) = (censo.as_ref(), por_cpuid) {
        let declarados = c.ids().len() as u32;
        if declarados != hilos {
            crate::ring0::cabina::warn(
                "smp",
                "el firmware declara otros hilos que el silicio (BIOS?)",
                declarados as u64,
            );
        }
        if c.apagados() > 0 {
            crate::ring0::cabina::warn(
                "smp",
                "hilos LISTADOS y no habilitados: no se llaman",
                c.apagados() as u64,
            );
        }
    }

    let esperados = match (censo.as_ref(), por_cpuid) {
        // Manda la MADT: es la lista de a quien se puede llamar.
        (Some(c), _) => (c.ids().len() as u32).saturating_sub(1),
        // Sin MADT se sigue, pero **diciendo que se esta suponiendo**.
        (None, Some(hilos)) if hilos > 1 => {
            crate::ring0::cabina::warn(
                "smp",
                "sin MADT: suponiendo APIC IDs 0..hilos-1",
                hilos as u64,
            );
            hilos - 1
        }
        _ => {
            crate::ring0::cabina::warn("smp", "sin MADT ni topologia: no se a quien llamar", 0);
            return (0, 0);
        }
    };

    // * MIRA Y NO TOQUES. Con `cuantos == 0` se sale AQUI, antes de resetear los
    // contadores y antes de escribir un solo byte en la memoria baja. Es lo que
    // hace que el caso por defecto sea de verdad inofensivo y no "inofensivo
    // salvo por lo que ya habia hecho al llegar".
    //
    // Y se contesta lo que hay ahora mismo: si ya se despertaron antes, este
    // camino lo dice sin volver a despertarlos.
    REHUSADO.store(false, Ordering::SeqCst);
    if cuantos == 0 {
        crate::ring0::core::dashboard::dashboard_log("[smp] censo pedido, no se desperto a nadie");
        // Y los que fallaron desde la ultima vez que alguien miro, dichos.
        let fallados = ficha::reportar_fallos();
        if fallados != 0 {
            crate::ring0::cabina::warn("smp", "obreros que tomaron una excepcion y estan parados", fallados as u64);
        }
        // ** AND THE CENSUS SAYS IF THEY ARE PARKED, because the count alone
        // was telling a true fact that answers a different question.
        //
        // Reported from metal on 2026-08-12: `smp stop` printed *"obreros
        // parados"* and the very next `smp` answered *"nucleos en pie: 12 de
        // 12"*. Both were correct -- `parar()` really does send them into a
        // permanent `cli; hlt`, and they really are still out of reset -- but
        // read together they say "stop did nothing", which is false.
        //
        // "En pie" counts CPUs that answered the SIPI. That number does not go
        // down when they stop working, and it should not: coming out of reset
        // is not the same as doing something. The fix is not a different count,
        // it is **saying the other half**.
        if crew::parados() {
            crate::ring0::cabina::warn(
                "smp",
                "los obreros estan PARADOS: en pie no es trabajando",
                tramp::VIVOS.load(Ordering::SeqCst) as u64,
            );
        }
        return (tramp::VIVOS.load(Ordering::SeqCst), esperados);
    }

    // ** WAKING SOMEBODY MEANS WANTING THEM TO WORK.
    //
    // `crew::reanudar()` existed and **nobody called it** -- pattern 24, again.
    // Consequence, and it was not theoretical: after a `smp stop`, `smp all`
    // would re-send INIT+SIPI, the APs would come back out of reset, report
    // themselves alive, enter `obrero()` -- and hit `if PARAR` on their first
    // line, parking forever. The census would have said `12 de 12` with not one
    // of them able to take work, and nothing would have said why.
    //
    // It goes HERE and not in the shell so that any caller gets it: whoever asks
    // for cores wants cores, and a wake that leaves them parked is not a wake.
    // ** NO SE DESPIERTA DOS VECES a quien ya trabaja (11-10, la auditoria
    // de `smp all`). Un INIT+SIPI a un obrero en pie lo saca de lo que hacia
    // (quiza de una parte en Ring 3) y lo hace llegar OTRA VEZ con un indice
    // nuevo: con menos pedidos que en pie, dos nucleos con el mismo indice
    // comparten TSS, pila de trap y salto, y el `rsp0 = 0` de uno es el
    // triple fallo del otro. Despues de `smp stop` (todos parados) si se
    // puede: para eso es `reanudar`.
    let en_pie = tramp::VIVOS.load(Ordering::SeqCst);
    if en_pie > 0 && (!crew::parados() || ring3::activa()) {
        crate::ring0::cabina::warn("smp", "ya hay obreros en pie: no se despiertan otra vez (reinicia para cambiar cuantos)", en_pie as u64);
        crate::ring0::core::dashboard::dashboard_log("[smp] ya hay obreros en pie: no se despiertan otra vez");
        REHUSADO.store(true, Ordering::SeqCst);
        return (en_pie, esperados);
    }
    crew::reanudar();

    // Nunca mas de los que caben en las fichas (`MAX_OBREROS`): uno de mas
    // llegaria sin ficha ni TSS (`tss.rs` no le da) y podria tomar partes.
    let pedidos = cuantos.min(esperados).min(ficha::MAX_OBREROS as u32);
    tramp::VIVOS.store(0, Ordering::SeqCst);
    tramp::MASCARA.store(0, Ordering::SeqCst);
    let yo = tramp::apic_id();
    // Quien es el BSP, para que un fallo sepa de que lado cayo.
    tramp::BSP_APIC.store(yo, Ordering::SeqCst);

    unsafe {
        // 1. El trampolin, a su pagina.
        let (ini, largo) = tramp::bytes();
        if largo == 0 || largo > 0x1000 {
            crate::ring0::cabina::fault("smp", "el trampolin mide algo imposible", largo as u64);
            return (0, esperados);
        }
        for i in 0..largo {
            core::ptr::write_volatile(
                (TRAMPOLIN as *mut u8).add(i),
                core::ptr::read_volatile(ini.add(i)),
            );
        }

        // 2. La GDT, dentro de la pagina de datos. El GDTR apunta a ESTA copia y
        //    no a la del kernel: en modo real la base es una direccion FISICA y
        //    tiene que caber en 32 bits.
        let gdt = (DATOS + OFF_GDT) as *mut u64;
        for (i, e) in GDT.iter().enumerate() {
            core::ptr::write_volatile(gdt.add(i), *e);
        }
        core::ptr::write_volatile((DATOS + OFF_GDTR) as *mut u16, (GDT.len() * 8 - 1) as u16);
        core::ptr::write_volatile((DATOS + OFF_GDTR + 2) as *mut u64, DATOS + OFF_GDT);

        // 3. La IDT: la del kernel, tal como la tiene puesta el BSP ahora mismo.
        let mut idtr = [0u8; 10];
        core::arch::asm!("sidt [{}]", in(reg) idtr.as_mut_ptr(), options(nostack));
        core::ptr::copy_nonoverlapping(idtr.as_ptr(), (DATOS + OFF_IDTR) as *mut u8, 10);

        // 4. CR3 del kernel y la entrada de 64 bits.
        core::ptr::write_volatile(
            (DATOS + OFF_CR3) as *mut u64,
            crate::ring0::mm::vmm::kernel_pml4(),
        );
        core::ptr::write_volatile(
            (DATOS + OFF_ENTRADA) as *mut u64,
            tramp::smp_ap_entrada as *const () as u64,
        );

        // 5. Y a llamar, de uno en uno. Cada AP recoge SU pila del mismo sitio,
        //    asi que hay que dejarla puesta antes de cada SIPI. De ahi que esto
        //    no sea un broadcast.
        //
        // La pila de cada AP se indexa por el ORDEN de la llamada, no por su
        // APIC ID: con la MADT esos IDs pueden ser cualquier cosa (en un x2APIC
        // son numeros grandes y dispersos), y `PILAS + id * 0x1000` se saldria
        // del primer MiB sin avisar. El orden siempre es 0, 1, 2...
        let mut slot = 0u64;
        let call = |id: u32, slot: &mut u64| {
            *slot += 1;
            core::ptr::write_volatile((DATOS + OFF_PILA) as *mut u64, PILAS + *slot * 0x1000);
            // Se dice ANTES de mandarlo, no despues: si el que cuelga es este,
            // el numero ya esta en pantalla.
            aviso(id);
            {
                use crate::ring0::cabina::tablero as t;
                t::marcar(t::SMP_BSP, t::LLAMA_AP, id as u64, *slot);
            }
            lapic::ipi(id, lapic::INIT);
            lapic::esperar_us(10_000);
            lapic::ipi(id, lapic::SIPI);
            lapic::esperar_us(200);
            lapic::ipi(id, lapic::SIPI);
            lapic::esperar_us(200);
        };

        // ** EN EL ORDEN DE LA FILA (11-10), no en el de la MADT: un hilo de
        // cada nucleo fisico ajeno primero, sus hermanos SMT despues y el
        // hermano del BSP al final (`topologia::fila`). Con `smp all` da
        // igual a quien se llame primero; con `smp 5` en un 5600X son CINCO
        // NUCLEOS FISICOS y no "los cinco primeros que lista la placa", que
        // con los hermanos juntos eran tres nucleos y el hermano del BSP.
        let mut fila_ids = [0u32; bmo_orquesta::topologia::HILOS_MAXIMOS];
        let por_fila = topologia::fila().map_or(0, |f| f.orden(yo, &mut fila_ids));
        match censo.as_ref() {
            Some(_) if por_fila > 0 => {
                for &id in &fila_ids[..por_fila] {
                    if id != yo && (slot as u32) < pedidos {
                        call(id, &mut slot);
                    }
                }
            }
            // * Sin fila, la lista del firmware tal cual. Ni se inventa ni se
            // ordena.
            Some(c) => {
                for &id in c.ids() {
                    if id != yo && (slot as u32) < pedidos {
                        call(id, &mut slot);
                    }
                }
            }
            // El camino de respaldo, ya avisado mas arriba.
            None => {
                for id in 0..esperados + 1 {
                    if id != yo && (slot as u32) < pedidos {
                        call(id, &mut slot);
                    }
                }
            }
        }
    }

    // 6. Contar, con tope. Un AP que no viene no puede colgar al que pregunta.
    // Se espera a los PEDIDOS, no a todos: con `smp 3` esperar a los once seria
    // colgar el comando un segundo entero para contar hasta tres.
    let mut vueltas = 0u32;
    while tramp::VIVOS.load(Ordering::SeqCst) < pedidos && vueltas < 1000 {
        lapic::esperar_us(1000);
        vueltas += 1;
    }

    let alive = tramp::VIVOS.load(Ordering::SeqCst);
    let mascara = tramp::MASCARA.load(Ordering::SeqCst);
    {
        use crate::ring0::cabina::tablero as t;
        t::marcar(t::SMP_BSP, t::CONTADOS, alive as u64, pedidos as u64);
    }

    // * TAMBIEN AL KLOG, y esto era un fallo mio de bulto: todo el relato del
    // bring-up iba **solo a CABINA**, que Ring 3 no puede leer. El mensaje del
    // compositor decia "(F11 lo cuenta entero)" y F11 muestra el KLOG, que es
    // otro sitio. O sea: se prometia un dato en una ventana donde no estaba.
    //
    // Son dos sumideros distintos a proposito --CABINA es el narrador con
    // severidad, el klog es la transcripcion-- y quien escribe tiene que elegir
    // los dos cuando quiere que se vea desde fuera.
    crate::ring0::core::dashboard::dashboard_log(if alive == pedidos {
        "[smp] contestaron todos los que se llamaron"
    } else {
        "[smp] FALTAN hilos por contestar"
    });

    decir_estado_cpu(alive);
    if alive == pedidos {
        crate::ring0::cabina::info("smp", "contestaron todos los llamados", alive as u64 + 1);
    } else {
        crate::ring0::cabina::warn(
            "smp",
            "faltan hilos por contestar",
            (pedidos - alive) as u64,
        );
        crate::ring0::cabina::warn("smp", "mascara de los que SI contestaron", mascara as u64);
    }
    (alive, esperados)
}

/// **El CR0 y el CR4 de los obreros contra los del BSP** (11-10). Lo que
/// importa: `CR0.CD` (bit 30, la cache APAGADA en ese nucleo), `CR0.NW` (29) y
/// `CR4.MCE` (6: sin el, una #MC es un apagado de la maquina entera). Se dice
/// el del obrero 0, el del BSP, y cuantos obreros tienen cada bit de riesgo.
fn decir_estado_cpu(vivos: u32) {
    let (bsp0, bsp4): (u64, u64);
    // SAFETY: leer CR0 y CR4 del BSP no cambia nada.
    unsafe {
        core::arch::asm!("mov {}, cr0", out(reg) bsp0, options(nomem, nostack));
        core::arch::asm!("mov {}, cr4", out(reg) bsp4, options(nomem, nostack));
    }
    let n = (vivos as usize).min(ficha::MAX_OBREROS);
    let cr0 = |i: usize| tramp::CR0_AP[i].load(Ordering::SeqCst);
    let cr4 = |i: usize| tramp::CR4_AP[i].load(Ordering::SeqCst);
    let sin_cache = (0..n).filter(|&i| cr0(i) & (3 << 29) != 0).count();
    let sin_mce = (0..n).filter(|&i| cr4(i) & (1 << 6) == 0).count();
    let distintos = (0..n).filter(|&i| cr0(i) != bsp0 || cr4(i) != bsp4).count();
    let byte = |x: usize| x.min(0xFF) as u64;
    VEREDICTO.store(1 << 63 | byte(n) | byte(distintos) << 8 | byte(sin_cache) << 16 | byte(sin_mce) << 24, Ordering::SeqCst);
    use crate::ring0::cabina::{info, warn};
    if n > 0 {
        info("smp", "CR0 del obrero 0", cr0(0));
        info("smp", "CR4 del obrero 0", cr4(0));
    }
    info("smp", "CR0 del BSP", bsp0);
    info("smp", "CR4 del BSP", bsp4);
    if distintos > 0 {
        warn("smp", "obreros con CR0/CR4 distintos del BSP", distintos as u64);
    }
    if sin_cache > 0 {
        warn("smp", "obreros con la CACHE apagada (CR0.CD o NW)", sin_cache as u64);
    }
    if sin_mce > 0 {
        warn("smp", "obreros sin CR4.MCE: una #MC alli REINICIA el PC", sin_mce as u64);
    }
    let encendidos = tramp::LLEGARON_SIN_CACHE.load(Ordering::SeqCst);
    if encendidos > 0 {
        warn("smp", "obreros que LLEGARON con la cache apagada (encendida al llegar)", encendidos as u64);
    }
}

/// `(alive, mascara)` sin volver a despertar nada.
pub fn alive() -> (u32, u32) {
    (
        tramp::VIVOS.load(Ordering::SeqCst),
        tramp::MASCARA.load(Ordering::SeqCst),
    )
}

// ============ ** EN QUE ESTA CADA NUCLEO, Y POR QUE ============
//
// === El hueco que tapa ===
//
// Hasta hoy la unica pregunta que este modulo sabia contestar era **cuantos**
// estan en pie. Y "once en pie" no dice nada de lo que importa: ni si estan
// trabajando, ni si estan girando en vacio, ni por que uno no contesto.
//
// > **Un nucleo despierto que no hace nada y uno que trabaja se miden igual
// > desde fuera: los dos estan "en pie".** Desde dentro tambien, hasta que
// > alguien escribe esta tabla.
//
// El motivo viaja con el estado a proposito. `nucleo 3: DORMIDO` sin el porque
// es exactamente lo que hace indepurable un sistema al mes siguiente -- y aqui
// los motivos no son adorno: **"parado por orden" y "no contesto al trampolin"
// se ven igual desde el contador y se arreglan en sitios opuestos**.

/// En que estado esta un nucleo. Ver `docs/maestro/AXION_MAESTRO.md`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Estado {
    /// El BSP. Propietario del kernel entero, y no se negocia.
    Maestro,
    /// En pie y aceptando faenas. **Gira** mientras espera: ver el coste.
    Obrero,
    /// Se le mando parar. Sin IPI **no vuelve**: ver `crew::parar`.
    Dormido,
    /// Se le llamo y no contesto. El fallo esta en el trampolin o en su pila.
    Ausente,
    /// Todavia no se ha llamado a nadie. No es un fallo: es que no se ha pedido.
    SinDespertar,
    /// **Existe y este kernel no puede decir en que esta.** El censo de vivos
    /// son 32 bits; a partir del nucleo 32 el bit se comparte. Ver `estado_de`.
    NoSeSabe,
}

impl Estado {
    pub fn nombre(self) -> &'static str {
        match self {
            Estado::Maestro => "MAESTRO",
            Estado::Obrero => "OBRERO",
            Estado::Dormido => "DORMIDO",
            Estado::Ausente => "AUSENTE",
            Estado::SinDespertar => "-",
            Estado::NoSeSabe => "?",
        }
    }

    /// La razon, en el idioma del sistema. **Es la mitad util de la tabla.**
    pub fn motivo(self) -> &'static str {
        match self {
            Estado::Maestro => "el kernel entero: drivers, CABINA, los static mut",
            Estado::Obrero => "acepta faenas; nunca toca un driver. GIRA al esperar",
            Estado::Dormido => "parado por orden; sin IPI no vuelve",
            Estado::Ausente => "se le llamo y no contesto: trampolin o pila",
            Estado::SinDespertar => "nadie lo ha llamado todavia",
            Estado::NoSeSabe => "existe, pero el censo solo cubre 32: ver tramp::MASCARA",
        }
    }
}

/// **El estado del nucleo con APIC id `id`.**
///
/// [!] El `id 0` es el BSP y se contesta sin mirar la mascara: es el nucleo que
/// esta ejecutando esta misma funcion. Preguntarselo a la mascara daria
/// `AUSENTE` --el BSP nunca pasa por el trampolin, asi que nunca pone su bit--
/// y el panel diria que el nucleo que lo esta pintando no existe.
pub fn estado_de(id: u32) -> Estado {
    // El BSP es el que es, no el APIC 0 (11-10): en una placa que arranque
    // por otro, el 0 seria un obrero pintado de maestro.
    let bsp = match tramp::BSP_APIC.load(Ordering::SeqCst) {
        u32::MAX => tramp::apic_id(),
        b => b,
    };
    if id == bsp {
        return Estado::Maestro;
    }
    let (vivos, mascara) = alive();
    if vivos == 0 {
        return Estado::SinDespertar;
    }
    // ** [!] HASTA 32 NUCLEOS, Y NI UNO MAS -- dicho, no supuesto.
    //
    // `MASCARA` son 32 bits y el bit se elige con `id & 31`, o sea que en una
    // maquina con mas de 32 hilos **el nucleo 33 comparte bit con el 1**. Su
    // cabecera en `tramp.rs` lo declara y dice *"ese problema todavia no
    // existe"* -- y era cierto en un 5600X de doce hilos.
    //
    // Deja de serlo en cuanto alguien arranque esto en un EPYC. Y lo que haria
    // esta funcion entonces no es fallar: es **contestar el estado de otro
    // nucleo** con toda tranquilidad, que es mucho peor.
    //
    // > Un limite que no se dice no es un limite: es una mentira esperando la
    // > maquina que la destape.
    if id >= 32 {
        return Estado::NoSeSabe;
    }
    if mascara & (1u32 << id) == 0 {
        return Estado::Ausente;
    }
    if crew::parados() {
        Estado::Dormido
    } else {
        Estado::Obrero
    }
}

/// **CORE o THREAD: es un nucleo fisico o el hermano SMT de otro?**
///
/// En ingles a peticion del propietario, y aqui las dos palabras hacen falta juntas:
/// `12 hilos` no dice si son doce nucleos o seis con SMT, y **la diferencia
/// decide el reparto** -- una faena de calculo denso quiere seis obreros, no
/// doce (ver `docs/maestro/AXION_MAESTRO.md`, apartado 3).
///
/// == De donde sale, y que se esta suponiendo ==
///
/// Del PERFIL, no de una tabla: si `hilos == nucleos * 2`, SMT esta encendido y
/// **el bit bajo del APIC ID separa a los dos hermanos** -- es como los numera
/// x86 cuando los IDs son contiguos, que es el caso de este Ryzen y de
/// cualquier maquina de un solo CCD.
///
/// [!] Y por eso `NO_SE` existe. Con IDs dispersos --x2APIC en una placa
/// grande-- el bit bajo deja de significar eso, y **contestar `CORE` a un
/// hermano SMT seria peor que no contestar**: el reparto creeria tener el doble
/// de unidades de ejecucion de las que hay. Lo correcto seria leer
/// `CPUID.8000001E`, y hasta que exista una maquina donde probarlo, esto se
/// calla en vez de adivinar.
/// **DONDE VIVE UN APIC ID: en que nucleo y en que hilo de ese nucleo.**
///
/// `None` cuando no se puede saber, y esa es la respuesta honesta: sin perfil,
/// o con un reparto de hilos que este juez no sabe leer, **no se inventa**.
///
/// # *** Por que esta pregunta esta AQUI y no en `ficha`
///
/// Porque `tipo_de` ya la contestaba a medias --*"CORE o THREAD"*-- y escribir
/// la version entera en otro fichero habria dejado **dos jueces del mismo
/// numero**, que es el patron 55 de este arbol y el que mas caro ha salido.
/// Ahora `tipo_de` es un caso de esto, no su gemelo.
///
/// [!] Y por eso lo llama el BSP y nunca un obrero: leer el perfil es tocar
/// estado del kernel, y el contrato de `crew` es que un AP no toca nada. La
/// ficha guarda el APIC --un numero suyo-- y la identidad se resuelve al
/// MIRARLA, que pasa en el BSP.
pub fn donde_vive(id: u32) -> Option<(u8, u8)> {
    // ** DE LA FILA DEL SILICIO (11-10), no de dividir lo que dice el perfil.
    // Antes: `id / (hilos / nucleos)`, que acierta en un 5600X solo si sus
    // APIC son 0..11 seguidos -- un supuesto. Ahora el nucleo es `apic >>
    // smt_bits` (CPUID 0x8000_001E, lo que lee `topologia::fila`) y el hilo
    // es su puesto DENTRO de ese nucleo entre los que existen: 0 el primero
    // (CORE), 1 su hermano SMT (THREAD). Con SMT apagado, todos son 0.
    let f = topologia::fila()?;
    let h = f.hilos().iter().find(|h| h.apic == id)?;
    let puesto = f.hilos().iter().filter(|o| o.nucleo == h.nucleo && o.hermano < h.hermano).count();
    Some((h.nucleo.min(255) as u8, puesto.min(255) as u8))
}

/// **El hilo `k` de la maquina** (11-10): los hilos que lista la placa,
/// ordenados por (nucleo fisico, puesto en el nucleo). `(apic, nucleo, puesto,
/// hilos de ese nucleo)`. Es lo que recorren los paneles: los APIC reales, no
/// `0..hilos` (que solo acierta si la placa los numera seguidos).
pub fn hilo_k(k: u32) -> Option<(u32, u32, u32, u32)> {
    let f = topologia::fila()?;
    let mut v = [(0u32, 0u32, 0u32); bmo_orquesta::topologia::HILOS_MAXIMOS];
    let n = f.hilos().len().min(v.len());
    for (s, h) in v.iter_mut().zip(f.hilos()) {
        *s = (h.nucleo, h.hermano, h.apic);
    }
    v[..n].sort_unstable();
    let &(nucleo, _, apic) = v[..n].get(k as usize)?;
    let puesto = v[..k as usize].iter().filter(|o| o.0 == nucleo).count() as u32;
    let del_nucleo = v[..n].iter().filter(|o| o.0 == nucleo).count() as u32;
    Some((apic, nucleo, puesto, del_nucleo))
}

/// Cuantos hilos lista la placa (sin repetidos). 0 si no hay MADT.
pub fn hilos_listados() -> u32 {
    topologia::fila().map_or(0, |f| f.hilos().len() as u32)
}

pub fn tipo_de(id: u32) -> &'static str {
    // ** Un caso del juez de arriba, no una segunda cuenta.
    match donde_vive(id) {
        Some((_, 0)) => "CORE",
        Some((_, _)) => "THREAD",
        None => "?",
    }
}

/// **Cuantos nucleos estan girando en vacio ahora mismo.**
///
/// Es el numero del ahorro, y hoy es incomodo a proposito: un obrero en espera
/// **gira** (`pause`), no duerme, asi que este contador es literalmente
/// *"cuantos nucleos estan al 100% sin hacer nada"*. Con los doce en pie son
/// once.
///
/// Existe para que el dia que entre `MWAIT` la mejora se pueda **medir** en vez
/// de suponerla. Ver el apartado 5 de `docs/maestro/AXION_MAESTRO.md`.
pub fn girando() -> u32 {
    if crew::parados() {
        0
    } else {
        alive().0
    }
}
