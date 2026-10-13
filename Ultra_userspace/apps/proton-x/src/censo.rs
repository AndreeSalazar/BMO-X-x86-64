//! **EL CENSO** (29-09; pieza propia desde el 13-10): `--censo <ruta>` no
//! ejecuta nada. Lee el `.exe` y sus DLL y dice cuales tiene la casa, cuales
//! faltan y de que clase son, en la consola y en `informe/censo.txt`. Salio
//! de `main.rs` entero: con el, `main.rs` pasaba de las 1.000 lineas de
//! codigo (L6a).

use super::*;

/// Una linea a la consola, y el turno al escritorio para que la drene: el
/// anillo de la consola hija son 2048 bytes y pierde lo MAS VIEJO.
fn linea(s: &str) {
    di(s);
    for _ in 0..4 {
        bmo::yield_screen();
    }
}

fn recortar(s: &str, n: usize) -> &str {
    &s[..s.len().min(n)]
}

/// Nombres de Windows que un byte suelto delante no puede volver nuevos.
const COMO_WINDOWS: &[&str] = &[
    "kernel32.dll",
    "user32.dll",
    "gdi32.dll",
    "ntdll.dll",
    "advapi32.dll",
    "shell32.dll",
];

/// Las clases de lo que se carga EN VIVO: (nombre, que significa, listar los
/// nombres en la consola).
type Clase = (&'static str, &'static str, bool);
const GRAFICOS: Clase = ("graficos", "", true);
const SONIDO: Clase = ("sonido", "", true);
const NVIDIA: Clase = ("driver nvidia", "", true);
const AMD: Clase = ("de AMD", "no hacen falta con la 3060", false);
const HERRAMIENTAS: Clase = (
    "herramientas",
    "depurar, capturar, perifericos: opcionales",
    false,
);
const API_SETS: Clase = (
    "api-ms",
    "alias de kernelbase/kernel32: la casa los resuelve alli",
    false,
);
const WINDOWS: Clase = ("de Windows", "", true);
const CLASES: &[Clase] = &[
    GRAFICOS,
    SONIDO,
    NVIDIA,
    WINDOWS,
    API_SETS,
    AMD,
    HERRAMIENTAS,
];

/// **La clase** de una DLL que se carga en vivo, por su nombre.
fn clase(d: &str) -> Clase {
    let n = d.to_ascii_lowercase();
    let empieza = |p: &[&str]| p.iter().any(|x| n.starts_with(x));
    if empieza(&["api-ms-", "ext-ms-"]) {
        API_SETS
    } else if empieza(&["d3d", "dxgi", "vulkan", "opengl"]) {
        GRAFICOS
    } else if empieza(&["xaudio", "dsound", "mss", "x3daudio", "xapofx", "mmdevapi"]) {
        SONIDO
    } else if empieza(&["nv", "gfn_", "physx3gpu", "cudart", "nvcuda"]) {
        NVIDIA
    } else if empieza(&["ati", "amd", "llvm_"]) {
        AMD
    } else if empieza(&[
        "renderdoc",
        "msdia",
        "srcsrv",
        "symaudit",
        "dbghelp",
        "rzchroma",
        "physxupdate",
        "gameoverlay",
        "steam_api",
        "bink",
    ]) {
        HERRAMIENTAS
    } else {
        WINDOWS
    }
}

/// **Lo que pesa una funcion** para el primer arranque del juego (el censo
/// maduro, 30-09: *"ignorar lo que no aporta"*). Del mas al menos: una se
/// queda con el MAS fuerte de los caminos por los que se pide.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Nivel {
    /// Importada por el `.exe` o por una DLL que se carga CON el: si falta
    /// UNA, el cargador no arranca (`bmo_proton_x::resolver`).
    Dura,
    /// `/DELAYLOAD`: se resuelve en su primera llamada, si la hay.
    Retrasada,
    /// De una DLL del juego que se carga EN VIVO (`LoadLibrary`): solo si la
    /// carga.
    Vivo,
    /// Solo la piden DLL que no hacen falta con la 3060 (de AMD) o que son
    /// herramientas (depurar, capturar, perifericos): fuera de las cuentas.
    NoAporta,
}

const NIVELES: [Nivel; 4] = [Nivel::Dura, Nivel::Retrasada, Nivel::Vivo, Nivel::NoAporta];

impl Nivel {
    fn nombre(self) -> &'static str {
        match self {
            Nivel::Dura => "DURAS",
            Nivel::Retrasada => "RETRASADAS",
            Nivel::Vivo => "EN VIVO",
            Nivel::NoAporta => "NO APORTAN",
        }
    }
}

/// **EL CENSO COMPLETO** (29-09; refinado: *"LISTAR por completo lo que
/// exige"*; maduro el 30-09: por NIVELES). Sin ejecutar nada:
///
/// ```text
///    el .exe y, RECURSIVAMENTE, cada DLL del juego que vive junto a el
///    todas las funciones de Windows que piden, SIN REPETIR, contra la casa,
///    cada una con su NIVEL: DURAS (sin ellas no arranca), RETRASADAS,
///    EN VIVO y NO APORTAN (ver `Nivel`)
///    las DLL que se cargan EN VIVO (sus nombres en los datos: d3d12, dxgi...)
/// ```
///
/// El numero que manda es el de las DURAS que faltan: con 0, el cargador
/// ya puede arrancar el juego (el primer contacto). Resumen por la consola;
/// la lista entera, en `informe/censo.txt`.
pub(super) fn censo(ruta: &[u8]) -> ! {
    use alloc::string::String;
    let nombre = core::str::from_utf8(ruta).unwrap_or("?");
    let Some(bloque) = bmo::Memoria::request(16 << 20) else {
        fin("sin memoria para el censo")
    };
    // SAFETY: el bloque es de este proceso y no se suelta nunca (forget).
    unsafe { MONTON.poner(bloque.base() as usize, 16 << 20, bloque.handle()) };
    core::mem::forget(bloque);
    // La carpeta del `.exe`: donde viven las DLL del juego.
    let dir: Vec<u8> = match ruta.iter().rposition(|&c| c == b'/') {
        Some(k) => ruta[..k + 1].to_vec(),
        None => Vec::new(),
    };
    let junto = |dll: &str| -> Vec<u8> {
        let mut r = dir.clone();
        r.extend_from_slice(dll.as_bytes());
        r
    };
    let existe = |dll: &str| bmo::Archivo::reflejar(&junto(dll)).is_ok();

    // Por mirar (DLL del juego, con su nivel) y ya mirados, sin mayusculas.
    let mut cola: Vec<(String, Nivel)> = Vec::new();
    let mut vistos: Vec<String> = Vec::new();
    // Las de Windows: (dll, funcion, la tiene la casa, nivel); sin repetir.
    let mut windows: Vec<(String, String, bool, Nivel)> = Vec::new();
    // Lo del juego: (fichero, MiB, importadas, retrasadas, nivel, fallo).
    let mut juego: Vec<(String, u64, usize, usize, Nivel, Option<String>)> = Vec::new();
    // Las que se cargan en vivo y no son del juego.
    let mut en_vivo: Vec<(String, String)> = Vec::new();

    let exe = match mirar(ruta) {
        Ok(m) => m,
        Err(f) => fin(&format!("censo: {nombre}: {f}")),
    };
    let la_imagen = imagen(&exe);
    let mut pendiente: Option<(String, Mirado, Nivel)> = Some((
        String::from(nombre.rsplit('/').next().unwrap_or(nombre)),
        exe,
        Nivel::Dura,
    ));
    loop {
        let Some((quien, m, nivel)) = pendiente.take() else {
            // La siguiente de la cola, la de nivel MAS fuerte: asi una DLL
            // se mira siempre con el mas fuerte de sus caminos (una de nivel
            // N solo pide de nivel N o mas flojo).
            let Some(i) = cola
                .iter()
                .enumerate()
                .min_by_key(|(_, c)| c.1)
                .map(|(i, _)| i)
            else {
                break;
            };
            let (dll, nv) = cola.remove(i);
            match mirar(&junto(&dll)) {
                Ok(m) => pendiente = Some((dll, m, nv)),
                Err(f) => juego.push((dll, 0, 0, 0, nv, Some(f))),
            }
            continue;
        };
        vistos.push(quien.to_ascii_lowercase());
        juego.push((
            quien.clone(),
            m.mide >> 20,
            m.imps.len(),
            m.retrasadas.len(),
            nivel,
            m.fallo_retrasadas
                .as_ref()
                .map(|f| format!("solo las retrasadas: {f}")),
        ));
        let mut encolar = |dll: &str, nv: Nivel| {
            if vistos.contains(&dll.to_ascii_lowercase()) {
                return;
            }
            match cola.iter_mut().find(|c| c.0.eq_ignore_ascii_case(dll)) {
                Some(c) => c.1 = c.1.min(nv),
                None => cola.push((String::from(dll), nv)),
            }
        };
        let retrasada = nivel.max(Nivel::Retrasada);
        let pedidas = m
            .imps
            .iter()
            .map(|i| (i, nivel))
            .chain(m.retrasadas.iter().map(|i| (i, retrasada)));
        for (i, nv) in pedidas {
            if existe(&i.dll) {
                encolar(&i.dll, nv);
                continue;
            }
            let f = format!("{}", i.funcion);
            match windows
                .iter_mut()
                .find(|(d, g, _, _)| d.eq_ignore_ascii_case(&i.dll) && *g == f)
            {
                Some(w) => w.3 = w.3.min(nv),
                None => {
                    let hay = bmo_proton_x_casa::tabla(&i.dll, &i.funcion).is_some();
                    windows.push((i.dll.clone(), f, hay, nv));
                }
            }
        }
        for d in &m.en_vivo {
            let importada = m
                .imps
                .iter()
                .chain(&m.retrasadas)
                .any(|i| i.dll.eq_ignore_ascii_case(d));
            if importada {
                continue;
            }
            if existe(d) {
                // Una DLL del juego que se carga en vivo: la de AMD o una
                // herramienta no aporta nada con la 3060.
                let c = clase(d);
                encolar(
                    d,
                    if c == AMD || c == HERRAMIENTAS {
                        Nivel::NoAporta
                    } else {
                        nivel.max(Nivel::Vivo)
                    },
                );
            } else if !en_vivo.iter().any(|(x, _)| x.eq_ignore_ascii_case(d)) {
                en_vivo.push((d.clone(), quien.clone()));
            }
        }
    }

    // -- Lo EN VIVO, limpio: fuera lo que ya se importa en algun sitio, los
    // nombres de menos de 3 letras (`s.dll`) y los que son otro con una letra
    // de mas delante (`Wkernel32.dll`: el byte de antes de la cadena).
    let importadas = |d: &str| windows.iter().any(|w| w.0.eq_ignore_ascii_case(d));
    let limpios: Vec<(String, String)> = en_vivo
        .iter()
        .filter(|(d, _)| {
            let raiz = d.len() - 4;
            let otro = en_vivo
                .iter()
                .any(|(x, _)| d.len() == x.len() + 1 && d[1..].eq_ignore_ascii_case(x))
                || COMO_WINDOWS
                    .iter()
                    .any(|x| d.len() == x.len() + 1 && d[1..].eq_ignore_ascii_case(x));
            raiz >= 3 && d.as_bytes()[0].is_ascii_alphabetic() && !otro && !importadas(d)
        })
        .cloned()
        .collect();

    // -- Las cuentas por nivel: (de, las tiene la casa).
    let cuenta = |nv: Nivel| {
        let de = windows.iter().filter(|w| w.3 == nv);
        (de.clone().count(), de.filter(|w| w.2).count())
    };
    // Por DLL de Windows, de un nivel: (dll, de, tiene), las que mas faltan antes.
    let por_dll = |nv: Nivel| {
        let mut dlls: Vec<(String, u32, u32)> = Vec::new();
        for (d, _, hay, _) in windows.iter().filter(|w| w.3 == nv) {
            match dlls.iter_mut().find(|x| x.0.eq_ignore_ascii_case(d)) {
                Some(x) => {
                    x.1 += 1;
                    x.2 += *hay as u32;
                }
                None => dlls.push((d.clone(), 1, *hay as u32)),
            }
        }
        dlls.sort_by(|a, b| (b.1 - b.2).cmp(&(a.1 - a.2)));
        dlls
    };

    // -- El resumen: CORTO (el anillo de la consola hija son 2048 bytes: el
    // primer censo completo perdio su cabecera) y linea a linea, cediendo el
    // turno para que el escritorio lo drene.
    // Un fichero con solo sus retrasadas sin leer SI se miro (cuenta lo demas).
    let parcial = |f: &Option<String>| {
        f.as_ref()
            .is_some_and(|f| f.starts_with("solo las retrasadas"))
    };
    let fallidos = juego
        .iter()
        .filter(|j| j.5.is_some() && !parcial(&j.5))
        .count();
    let a_medias = juego.iter().filter(|j| parcial(&j.5)).count();
    linea(&format!(
        "CENSO de {nombre}: {} ficheros del juego ({} MiB); {} funciones de Windows distintas\n",
        juego.len() - fallidos,
        juego.iter().map(|j| j.1).sum::<u64>(),
        windows.len()
    ));
    linea(&la_imagen);
    let (de, si) = cuenta(Nivel::Dura);
    linea(&format!(
        "  PARA ARRANCAR (DURAS): {de}; la casa tiene {si} ({}%), FALTAN {}\n",
        si * 100 / de.max(1),
        de - si
    ));
    for nv in [Nivel::Retrasada, Nivel::Vivo] {
        let (de, si) = cuenta(nv);
        let que = if nv == Nivel::Retrasada {
            "solo si las llama"
        } else {
            "solo si carga su DLL"
        };
        linea(&format!(
            "  {:<11} {de:>5}; faltan {:>4}  ({que})\n",
            nv.nombre(),
            de - si
        ));
    }
    let (de, _) = cuenta(Nivel::NoAporta);
    linea(&format!(
        "  NO APORTAN  {de:>5}  (solo de AMD o de herramientas: fuera de las cuentas)\n"
    ));
    let duras = por_dll(Nivel::Dura);
    if duras.iter().all(|d| d.1 == d.2) {
        linea("  NO FALTA NINGUNA DURA: el cargador ya puede arrancar el juego (el primer contacto)\n");
    } else {
        linea("  las DLL de Windows con mas DURAS que faltan:\n");
        for par in duras
            .iter()
            .filter(|d| d.1 > d.2)
            .take(12)
            .collect::<Vec<_>>()
            .chunks(2)
        {
            let mut l = String::new();
            for (d, n, si) in par {
                l.push_str(&format!(
                    "  {:<22} {:>4} de {:<4}",
                    recortar(d, 22),
                    n - si,
                    n
                ));
            }
            l.push('\n');
            linea(&l);
        }
        let resto = duras
            .iter()
            .filter(|d| d.1 > d.2)
            .count()
            .saturating_sub(12);
        if resto > 0 {
            linea(&format!(
                "  ... y {resto} DLL mas con alguna DURA que falta\n"
            ));
        }
    }
    linea(&format!(
        "  EN VIVO (LoadLibrary, sin importarse): {} DLL\n",
        limpios.len()
    ));
    for c in CLASES {
        let de: Vec<&(String, String)> = limpios.iter().filter(|(d, _)| clase(d) == *c).collect();
        if de.is_empty() {
            continue;
        }
        let mut l = format!("    {:<14} {:>3}", c.0, de.len());
        if c.2 {
            l.push(' ');
            for (d, _) in de.iter().take(10) {
                l.push(' ');
                l.push_str(&d[..d.len() - 4]);
            }
            if de.len() > 10 {
                l.push_str(" ...");
            }
        } else {
            l.push_str("  ");
            l.push_str(c.1);
        }
        l.push('\n');
        linea(&l);
    }
    if a_medias > 0 {
        linea(&format!("  {a_medias} ficheros del juego sin sus retrasadas (lo demas cuenta; en informe/censo.txt)\n"));
    }
    if fallidos > 0 {
        linea(&format!(
            "  {fallidos} ficheros del juego no se pudieron mirar (en informe/censo.txt)\n"
        ));
    }
    linea(&format!(
        "  la lista entera, funcion a funcion: {}\n",
        core::str::from_utf8(RUTA_CENSO).unwrap_or("")
    ));

    // -- La lista entera, nivel a nivel.
    let mut t = String::new();
    let (de, si) = cuenta(Nivel::Dura);
    t.push_str(&format!("# CENSO de {nombre}\n# {} funciones de Windows distintas; PARA ARRANCAR (DURAS) {de}, la casa tiene {si}, FALTAN {}\n", windows.len(), de - si));
    t.push_str(&format!("#{}", &la_imagen[1..]));
    for nv in NIVELES {
        let (de, si) = cuenta(nv);
        t.push_str(&format!(
            "#   {:<11} {de:>5}  faltan {:>5}\n",
            nv.nombre(),
            de - si
        ));
    }
    for nv in NIVELES {
        t.push_str(&format!(
            "\n## {}: POR DLL DE WINDOWS (faltan, de, dll)\n",
            nv.nombre()
        ));
        let dlls = por_dll(nv);
        for (d, n, si) in &dlls {
            t.push_str(&format!("{:>5} {:>5} {d}\n", n - si, n));
        }
        t.push_str(&format!("\n## {}: FALTAN (dll funcion)\n", nv.nombre()));
        for (d, _, _) in &dlls {
            for (e, f, hay, n) in &windows {
                if !hay && *n == nv && e.eq_ignore_ascii_case(d) {
                    t.push_str(&format!("{e} {f}\n"));
                }
            }
        }
    }
    t.push_str("\n## LA CASA YA LAS TIENE (nivel dll funcion)\n");
    for (e, f, hay, n) in &windows {
        if *hay {
            t.push_str(&format!("{} {e} {f}\n", n.nombre()));
        }
    }
    t.push_str("\n## EN VIVO (clase dll <- nombre visto en los datos de)\n");
    for c in CLASES {
        for (d, quien) in limpios.iter().filter(|(d, _)| clase(d) == *c) {
            t.push_str(&format!("{} {d} <- {quien}\n", c.0));
        }
    }
    t.push_str("\n## EN VIVO DESCARTADOS (ya importados, o ruido)\n");
    for (d, quien) in en_vivo.iter().filter(|x| !limpios.contains(x)) {
        t.push_str(&format!("{d} <- {quien}\n"));
    }
    t.push_str("\n## FICHEROS DEL JUEGO (nivel, MiB, importadas, retrasadas)\n");
    for (f, mib, n, r, nv, fallo) in &juego {
        match fallo {
            None => t.push_str(&format!("{} {f} {mib} {n} {r}\n", nv.nombre())),
            Some(e) if parcial(fallo) => {
                t.push_str(&format!("{} {f} {mib} {n} {r} ({e})\n", nv.nombre()))
            }
            Some(e) => t.push_str(&format!("{} {f} NO SE PUDO MIRAR: {e}\n", nv.nombre())),
        }
    }
    guardar_censo(t.as_bytes());
    bmo::salir();
}

/// **Lo que mide la imagen del `.exe`** (P0.4a). Desde P0.4b va en una
/// imagen DECLARADA (`bmo::Imagen`): ya no hay tope de 64 MiB por parte, y lo
/// que decide es la RAM libre cuando se declara.
fn imagen(m: &Mirado) -> alloc::string::String {
    let mib = |x: u64| x.div_ceil(1 << 20);
    match &m.partes {
        Err(f) => format!("  LA IMAGEN: no se puede partir ({f})\n"),
        Ok(p) => {
            let suma = |cod: bool| {
                p.iter()
                    .filter(|t| t.codigo == cod)
                    .map(|t| t.bytes as u64)
                    .sum::<u64>()
            };
            let (c, d) = (suma(true), suma(false));
            let veredicto = "va en la imagen declarada (la juzga el kernel contra la RAM libre)";
            format!("  LA IMAGEN: {} MiB en el disco; en memoria {} MiB = codigo {} + datos {}; {veredicto}\n", mib(m.mide), mib(c + d), mib(c), mib(d))
        }
    }
}

/// La lista entera a `informe/censo.txt`, de una llamada.
fn guardar_censo(bytes: &[u8]) {
    let Ok(f) = bmo::Archivo::create(RUTA_CENSO) else {
        di("  (no se pudo crear informe/censo.txt)\n");
        return;
    };
    let escritos = match bmo::Memoria::request(bytes.len().max(1) as u64) {
        Some(b) => {
            // SAFETY: un bloque nuestro de al menos `bytes.len()` bytes.
            unsafe { core::ptr::copy_nonoverlapping(bytes.as_ptr(), b.base(), bytes.len()) };
            f.escribir_de(&b, 0, bytes.len() as u64) as usize
        }
        None => f.write(bytes),
    };
    if escritos != bytes.len() || !f.close() {
        di("  (informe/censo.txt no salio entero)\n");
    }
}
