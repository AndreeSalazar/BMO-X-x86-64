//! **El turno PRESTADO al sonido** (T1 y T2 de `PLAN_LOS_DOCE_DIRECTORES`,
//! 07-10), con hilos de la casa de verdad: el principal hace algo LARGO (una
//! lista de dibujo, mil PSO) sin esperar nada; el hilo del sonido espera el
//! evento de su `IAudioClient`; otro hilo del juego esta listo. Respirando,
//! el del sonido corre a mitad de lo largo y el otro NO (entraria en D3D12 a
//! mitad de la lista). Sin respirar, el del sonido no corre: los 96 cortes
//! de Cyberpunk en el metal (06-10).

use super::*;
use bmo_proton_x_casa::com::{guid, Guid};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64};

static EV_SONIDO: AtomicU64 = AtomicU64::new(0);
static EV_OTRO: AtomicU64 = AtomicU64::new(0);
static VECES: AtomicU32 = AtomicU32::new(0);
static FIN: AtomicBool = AtomicBool::new(false);
static OTRO_CORRIO: AtomicBool = AtomicBool::new(false);

type Esperar = extern "win64" fn(u64, u32) -> u32;
type Encender = extern "win64" fn(u64) -> i32;

fn f(d: &str, n: &str) -> usize {
    bmo_proton_x_casa::tabla(d, &bmo_proton_x::Funcion::Nombre(n.into())).unwrap_or_else(|| panic!("{n}")) as usize
}

fn esperar(h: u64) -> u32 {
    // SAFETY: WaitForSingleObject de la casa.
    let e: Esperar = unsafe { core::mem::transmute(f("kernel32.dll", "WaitForSingleObject")) };
    e(h, u32::MAX)
}

/// El hilo del sonido de un motor (Wwise): espera su evento y rellena.
extern "win64" fn hilo_del_sonido(_: u64) -> u32 {
    loop {
        esperar(EV_SONIDO.load(Ordering::SeqCst));
        if FIN.load(Ordering::SeqCst) {
            return 0;
        }
        VECES.fetch_add(1, Ordering::SeqCst);
    }
}

/// Otro hilo del juego: espera un evento normal; cuando corre, lo apunta.
extern "win64" fn otro_hilo(_: u64) -> u32 {
    esperar(EV_OTRO.load(Ordering::SeqCst));
    OTRO_CORRIO.store(true, Ordering::SeqCst);
    0
}

/// El hueco `h` de la vtabla de un objeto de la casa, llamado con `a`.
fn metodo(o: u64, h: usize, a: &[u64]) -> i32 {
    // SAFETY: un objeto de la casa: su primer puntero es la vtabla, y el
    // hueco `h` tiene esa forma (la prueba lo sabe por la interfaz).
    unsafe {
        let p = (*(o as *const *const usize)).add(h).read();
        match a.len() {
            0 => core::mem::transmute::<usize, extern "win64" fn(u64) -> i32>(p)(o),
            1 => core::mem::transmute::<usize, extern "win64" fn(u64, u64) -> i32>(p)(o, a[0]),
            3 => core::mem::transmute::<usize, extern "win64" fn(u64, u64, u64, u64) -> i32>(p)(o, a[0], a[1], a[2]),
            _ => core::mem::transmute::<usize, extern "win64" fn(u64, u64, u64, u64, u64) -> i32>(p)(o, a[0], a[1], a[2], a[3]),
        }
    }
}

#[test]
fn lo_largo_presta_el_turno_al_sonido_y_a_nadie_mas() {
    let _uno = uno_a_la_vez();
    DICHO.lock().unwrap().clear();
    unsafe { bmo_proton_x_casa::empezar(plataforma()) };
    // Un TEB en el GS, como `correr_exe` (CreateThread copia el suyo).
    let bytes = (teb::TEB_BYTES + teb::PEB_BYTES) as u64;
    let mem = mmap(bytes);
    let rsp: u64;
    unsafe { core::arch::asm!("mov {}, rsp", out(reg) rsp) };
    let h = teb::Hilo { teb: mem, peb: mem + teb::TEB_BYTES as u64, pila_tope: rsp + (64 << 10), pila_fondo: rsp - (256 << 10), proceso: 7, hilo: 42, base_imagen: 0 };
    {
        let t = unsafe { core::slice::from_raw_parts_mut(mem as *mut u8, bytes as usize) };
        let (tb, pb) = t.split_at_mut(teb::TEB_BYTES);
        teb::escribir_teb(tb, &h);
        teb::escribir_peb(pb, &h);
    }
    poner_gs(mem);
    struct Quitar(u64, u64);
    impl Drop for Quitar {
        fn drop(&mut self) {
            poner_gs(0);
            munmap(self.0, self.1);
        }
    }
    let _quitar = Quitar(mem, bytes);

    // El motor de sonido: el aparato por defecto, un IAudioClient POR
    // EVENTO de 20 ms, su evento, Start.
    const CLSID_ENUMERADOR: Guid = guid(0xBCDE_0395, 0xE52F, 0x467C, [0x8E, 0x3D, 0xC4, 0x57, 0x92, 0x91, 0x69, 0x2E]);
    const IID_ENUMERADOR: Guid = guid(0xA956_64D2, 0x9614, 0x4F35, [0xA7, 0x46, 0xDE, 0x8D, 0xB6, 0x36, 0x17, 0xE6]);
    const IID_CLIENTE: Guid = guid(0x1CB9_AD4C, 0xDBFA, 0x4C32, [0xB1, 0x78, 0xC2, 0xF5, 0x68, 0xA7, 0x03, 0xB2]);
    type Crear = extern "win64" fn(*const Guid, u64, u32, *const Guid, *mut u64) -> u32;
    type Iniciar = extern "win64" fn(u64, u32, u32, i64, i64, *const u8, *const Guid) -> i32;
    type CrearEvento = extern "win64" fn(u64, i32, i32, u64) -> u64;
    type CrearHilo = extern "win64" fn(u64, usize, u64, u64, u32, *mut u32) -> u64;
    let crear: Crear = unsafe { core::mem::transmute(f("ole32.dll", "CoCreateInstance")) };
    let crear_evento: CrearEvento = unsafe { core::mem::transmute(f("kernel32.dll", "CreateEventW")) };
    let crear_hilo: CrearHilo = unsafe { core::mem::transmute(f("kernel32.dll", "CreateThread")) };
    let encender: Encender = unsafe { core::mem::transmute(f("kernel32.dll", "SetEvent")) };
    let ceder: extern "win64" fn() -> i32 = unsafe { core::mem::transmute(f("kernel32.dll", "SwitchToThread")) };
    let (mut en, mut ap, mut cliente) = (0u64, 0u64, 0u64);
    assert_eq!(crear(&CLSID_ENUMERADOR, 0, 1, &IID_ENUMERADOR, &mut en), 0);
    assert_eq!(metodo(en, 4, &[0, 0, &mut ap as *mut u64 as u64]), 0, "la salida por defecto");
    assert_eq!(metodo(ap, 3, &[&IID_CLIENTE as *const Guid as u64, 1, 0, &mut cliente as *mut u64 as u64]), 0, "Activate(IAudioClient)");
    let mezcla = bmo_proton_x::pcm::Formato::mezcla(48_000).a_waveformatextensible();
    let iniciar: Iniciar = unsafe { core::mem::transmute((*(cliente as *const *const usize)).add(3).read()) };
    assert_eq!(iniciar(cliente, 0, 0x0004_0000, 200_000, 0, mezcla.as_ptr(), core::ptr::null()), 0, "20 ms, por evento");
    EV_SONIDO.store(crear_evento(0, 0, 0, 0), Ordering::SeqCst);
    EV_OTRO.store(crear_evento(0, 1, 0, 0), Ordering::SeqCst);
    assert_eq!(metodo(cliente, 13, &[EV_SONIDO.load(Ordering::SeqCst)]), 0, "SetEventHandle");
    assert_eq!(metodo(cliente, 10, &[]), 0, "Start");
    FIN.store(false, Ordering::SeqCst);
    OTRO_CORRIO.store(false, Ordering::SeqCst);
    let mut tid = 0u32;
    let h_sonido = crear_hilo(0, 0, hilo_del_sonido as extern "win64" fn(u64) -> u32 as usize as u64, 0, 0, &mut tid);
    let h_otro = crear_hilo(0, 0, otro_hilo as extern "win64" fn(u64) -> u32 as usize as u64, 0, 0, &mut tid);
    assert!(h_sonido != 0 && h_otro != 0);
    // Los dos llegan a su espera (el del sonido quiza ya relleno alguna vez).
    ceder();
    ceder();
    // El otro ya PUEDE seguir: un hilo del juego listo, como en Cyberpunk.
    encender(EV_OTRO.load(Ordering::SeqCst));
    let base = VECES.load(Ordering::SeqCst);
    let prestados = bmo_proton_x_casa::hilos::prestamos();

    // La prueba que dice NO: 60 ms de algo largo SIN respirar. El evento
    // del sonido toca tres veces en ese tiempo, y su hilo no corre ni una.
    let t0 = std::time::Instant::now();
    while t0.elapsed().as_millis() < 60 {
        std::hint::spin_loop();
    }
    assert_eq!(VECES.load(Ordering::SeqCst), base, "sin respirar, el sonido no corre");

    // Lo largo RESPIRA (cada orden de una lista): el del sonido rellena a
    // mitad, y el turno vuelve aqui.
    let t0 = std::time::Instant::now();
    while VECES.load(Ordering::SeqCst) < base + 3 && t0.elapsed().as_secs() < 5 {
        bmo_proton_x_casa::hilos::respirar();
    }
    assert!(VECES.load(Ordering::SeqCst) >= base + 3, "respirando, el sonido rellena: {} veces", VECES.load(Ordering::SeqCst) - base);
    assert!(bmo_proton_x_casa::hilos::prestamos() >= prestados + 3);
    assert!(!OTRO_CORRIO.load(Ordering::SeqCst), "el otro hilo del juego, listo, NO corrio a mitad de lo largo");

    // Y al terminar lo largo, la rueda de siempre: el otro corre.
    assert_eq!(esperar(h_otro), 0);
    assert!(OTRO_CORRIO.load(Ordering::SeqCst));
    FIN.store(true, Ordering::SeqCst);
    encender(EV_SONIDO.load(Ordering::SeqCst));
    assert_eq!(esperar(h_sonido), 0);
}

static LISTO: AtomicBool = AtomicBool::new(false);
static CERROJO: [u64; 8] = [0; 8];

/// El que avisa: no espera nada, solo pone la bandera (y para eso necesita
/// el turno).
extern "win64" fn el_que_avisa(_: u64) -> u32 {
    LISTO.store(true, Ordering::SeqCst);
    0
}

/// *** EL CUANTO (07-10, el metal: Cyberpunk parado antes de su primer
/// Present con un hilo que entraba y salia de cerrojos sin esperar nunca y
/// seis listos sin turno). El principal da vueltas con Enter/Leave de una
/// seccion critica hasta que otro hilo ponga una bandera: sin el cuanto, el
/// otro no corre NUNCA (la prueba se colgaria); con el, en unos ms.
#[test]
fn el_que_da_vueltas_con_cerrojos_suelta_el_turno() {
    let _uno = uno_a_la_vez();
    DICHO.lock().unwrap().clear();
    unsafe { bmo_proton_x_casa::empezar(plataforma()) };
    let bytes = (teb::TEB_BYTES + teb::PEB_BYTES) as u64;
    let mem = mmap(bytes);
    let rsp: u64;
    unsafe { core::arch::asm!("mov {}, rsp", out(reg) rsp) };
    let h = teb::Hilo { teb: mem, peb: mem + teb::TEB_BYTES as u64, pila_tope: rsp + (64 << 10), pila_fondo: rsp - (256 << 10), proceso: 7, hilo: 42, base_imagen: 0 };
    {
        let t = unsafe { core::slice::from_raw_parts_mut(mem as *mut u8, bytes as usize) };
        let (tb, pb) = t.split_at_mut(teb::TEB_BYTES);
        teb::escribir_teb(tb, &h);
        teb::escribir_peb(pb, &h);
    }
    poner_gs(mem);
    struct Quitar(u64, u64);
    impl Drop for Quitar {
        fn drop(&mut self) {
            poner_gs(0);
            munmap(self.0, self.1);
        }
    }
    let _quitar = Quitar(mem, bytes);
    type CrearHilo = extern "win64" fn(u64, usize, u64, u64, u32, *mut u32) -> u64;
    type Cerrojo = extern "win64" fn(u64);
    let crear_hilo: CrearHilo = unsafe { core::mem::transmute(f("kernel32.dll", "CreateThread")) };
    let entrar: Cerrojo = unsafe { core::mem::transmute(f("kernel32.dll", "EnterCriticalSection")) };
    let salir: Cerrojo = unsafe { core::mem::transmute(f("kernel32.dll", "LeaveCriticalSection")) };
    LISTO.store(false, Ordering::SeqCst);
    let antes = bmo_proton_x_casa::hilos::cuantos();
    let mut tid = 0u32;
    let h_otro = crear_hilo(0, 0, el_que_avisa as extern "win64" fn(u64) -> u32 as usize as u64, 0, 0, &mut tid);
    assert!(h_otro != 0);
    let cs = CERROJO.as_ptr() as u64;
    let t0 = std::time::Instant::now();
    while !LISTO.load(Ordering::SeqCst) && t0.elapsed().as_secs() < 5 {
        entrar(cs);
        std::hint::spin_loop();
        salir(cs);
    }
    assert!(LISTO.load(Ordering::SeqCst), "el otro hilo corrio dentro de las vueltas con cerrojos");
    assert!(t0.elapsed().as_millis() < 1000, "en unos ms, no al tope: {:?}", t0.elapsed());
    assert!(bmo_proton_x_casa::hilos::cuantos() > antes, "lo solto el cuanto");
    assert_eq!(esperar(h_otro), 0);
}
