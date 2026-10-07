//! **LOS JUECES DE PROTON-X, SOLOS** (06-10) -- `run sys/jueces.bex`.
//!
//! El propietario: *"automatice la parte, porque me dara flojera escribir
//! todos en BMO-X"*. Hasta hoy cada juez se lanzaba a mano (`run
//! sys/proton-x.bex window/<nombre>.exe`), 84 veces, mirando la consola.
//! Esto los lanza uno tras otro, cada uno con SU consola (la salida no se
//! mezcla con la del sistema), lee lo que dicen, los cuenta y apunta todo en
//! `informe/jueces.txt`. Es el `correr_en_windows.ps1` de BMO-X, con la MISMA
//! tabla (`platform/shared/proton-x/prueba/jueces.txt`, dentro de este
//! binario): lo que Windows dijo bueno, aqui tiene que salir igual.
//!
//! ```text
//!    run sys/jueces.bex                  los 84
//!    run sys/jueces.bex d3d12            un grupo: dentro, tandas o d3d12
//!    run sys/jueces.bex olas tipos       los que se nombran
//!    run sys/jueces.bex d3d12 tope=900   segundos por juez (600 si no)
//! ```
//!
//! Por juez, `bien` si dice lo que pide la tabla (sus `bien`, ningun `MAL`,
//! sus `nota` si se cuentan, y sale con 0), `DISTINTO` y por que si no, y
//! `COLGADO` si pasa del tope (se cierra y se sigue con el siguiente). Los
//! avisos `PROTON-X:` no cuentan para nada: van al informe, que es donde se
//! leen.
//!
//! [consumo] NADA  solo mientras corre; espera a cada hijo cediendo el turno

#![no_std]
#![no_main]

use bmo_userland as bmo;

/// La tabla de los jueces, la de Windows.
const TABLA: &str = include_str!("../../../../platform/shared/proton-x/prueba/jueces.txt");
const RUTA_INFORME: &[u8] = b"informe/jueces.txt";
/// Segundos por juez si no se dice otro tope: un juez de D3D12 corre en la
/// CPU, y en el metal nadie lo ha medido aun.
const TOPE_S: u64 = 600;
/// Lo que se guarda del informe y de la salida de UN juez.
const INFORME_BYTES: usize = 256 * 1024;
const SALIDA_BYTES: usize = 32 * 1024;

/// Un juez de la tabla.
struct Juez<'a> {
    nombre: &'a str,
    grupo: &'a str,
    bien: i32,
    notas: i32,
    otra: Option<(i32, i32)>,
    que: &'a str,
}

/// La linea `nombre bien [nota [otra_bien otra_nota]] # que`.
fn leer_juez<'a>(linea: &'a str, grupo: &'a str) -> Option<Juez<'a>> {
    let (campos, que) = match linea.find('#') {
        Some(k) => (&linea[..k], linea[k + 1..].trim()),
        None => (linea, ""),
    };
    let mut c = campos.split_whitespace();
    let nombre = c.next()?;
    let num = |s: Option<&str>| s.and_then(|x| x.parse::<i32>().ok());
    let bien = num(c.next())?;
    let notas = num(c.next()).unwrap_or(-1);
    let otra = match (num(c.next()), num(c.next())) {
        (Some(a), Some(b)) => Some((a, b)),
        _ => None,
    };
    Some(Juez { nombre, grupo, bien, notas, otra, que })
}

/// Un texto que se va llenando en un bloque nuestro (sin monton).
struct Texto {
    p: *mut u8,
    cabe: usize,
    n: usize,
    _bloque: bmo::Memoria,
}

impl Texto {
    fn nuevo(bytes: usize) -> Option<Texto> {
        let b = bmo::Memoria::request(bytes as u64)?;
        Some(Texto { p: b.base(), cabe: bytes, n: 0, _bloque: b })
    }
    fn poner(&mut self, s: &[u8]) {
        let k = s.len().min(self.cabe - self.n);
        // SAFETY: `n + k <= cabe` bytes de nuestro bloque.
        unsafe { core::ptr::copy_nonoverlapping(s.as_ptr(), self.p.add(self.n), k) };
        self.n += k;
    }
    fn texto(&self) -> &[u8] {
        // SAFETY: los `n` primeros bytes, escritos por `poner`.
        unsafe { core::slice::from_raw_parts(self.p, self.n) }
    }
    fn vaciar(&mut self) {
        self.n = 0;
    }
}

impl core::fmt::Write for Texto {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.poner(s.as_bytes());
        Ok(())
    }
}

/// Lo que se va diciendo, a la consola de quien me lanzo.
struct Pantalla;

impl core::fmt::Write for Pantalla {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        bmo::consola(s);
        Ok(())
    }
}

macro_rules! di {
    ($($t:tt)*) => {{
        use core::fmt::Write;
        let _ = write!(Pantalla, $($t)*);
    }};
}

fn ahora_s(hz: u64) -> u64 {
    bmo::ciclos() / hz.max(1)
}

/// Lo que dijo un juez, contado de su salida.
#[derive(Default)]
struct Cuenta {
    bien: i32,
    mal: i32,
    nota: i32,
    avisos: i32,
    /// `PROTON-X: el .exe salio con N`.
    salio: Option<u32>,
    colgado: bool,
}

fn empieza(linea: &[u8], palabra: &[u8]) -> bool {
    let t = linea.iter().position(|&c| c != b' ').map_or(&linea[..0], |k| &linea[k..]);
    t.starts_with(palabra) && t.get(palabra.len()).is_none_or(|&c| c == b' ')
}

fn contar(linea: &[u8], c: &mut Cuenta) {
    const SALIO: &[u8] = b"PROTON-X: el .exe salio con ";
    if empieza(linea, b"bien") {
        c.bien += 1;
    } else if empieza(linea, b"MAL") {
        c.mal += 1;
    } else if empieza(linea, b"nota") {
        c.nota += 1;
    } else if let Some(r) = linea.strip_prefix(SALIO) {
        c.salio = core::str::from_utf8(r).ok().and_then(|s| s.trim().parse().ok());
    } else if linea.starts_with(b"PROTON-X:") {
        c.avisos += 1;
    }
}

/// Las lineas que van al informe aunque el juez diga bien: los MAL, las
/// notas, los avisos y los NO de la casa.
fn se_apunta(linea: &[u8]) -> bool {
    empieza(linea, b"MAL") || empieza(linea, b"nota") || linea.starts_with(b"PROTON-X:")
}

/// **Un juez**: lanzado con su consola, leido hasta que muere (o hasta el
/// tope), contado.
fn correr(j: &Juez, tope_s: u64, hz: u64, salida: &mut Texto) -> Cuenta {
    let mut c = Cuenta::default();
    salida.vaciar();
    let Some(consola) = bmo::Consola::create() else {
        salida.poner(b"(sin consola para el hijo)\n");
        return c;
    };
    let mut orden = [0u8; 64];
    let mut n = 0;
    for parte in [b"sys/proton-x.bex window/".as_slice(), j.nombre.as_bytes(), b".exe"] {
        let k = parte.len().min(orden.len() - n);
        orden[n..n + k].copy_from_slice(&parte[..k]);
        n += k;
    }
    let tid = match bmo::ejecutar_en(&orden[..n], consola.cap) {
        Ok(t) => t,
        Err(e) => {
            let _ = core::fmt::Write::write_fmt(salida, format_args!("(no se pudo lanzar: codigo {e})\n"));
            return c;
        }
    };
    let hijo = bmo::Hijo::por_tid(tid as u32);
    let desde = ahora_s(hz);
    let mut linea = [0u8; 512];
    let mut largo = 0usize;
    let mut quietos = 0u32;
    loop {
        let mut b = [0u8; 8];
        let k = consola.read(&mut b);
        if k > 0 {
            quietos = 0;
            for &x in &b[..k] {
                if x == b'\n' {
                    contar(&linea[..largo], &mut c);
                    salida.poner(&linea[..largo]);
                    salida.poner(b"\n");
                    largo = 0;
                } else if x != b'\r' && largo < linea.len() {
                    linea[largo] = x;
                    largo += 1;
                }
            }
            continue;
        }
        let vive = hijo.as_ref().is_some_and(|h| h.vive());
        if !vive {
            // Lo que quede en el anillo: dos vueltas sin nada, y se acabo.
            quietos += 1;
            if quietos > 2 {
                break;
            }
        } else if ahora_s(hz).saturating_sub(desde) > tope_s {
            if let Some(h) = &hijo {
                h.cerrar();
            }
            c.colgado = true;
            break;
        }
        bmo::wait(0, 0, 2_000_000);
    }
    if largo > 0 {
        contar(&linea[..largo], &mut c);
        salida.poner(&linea[..largo]);
        salida.poner(b"\n");
    }
    c
}

/// Si dice lo que pide la tabla; si no, por que (en `por_que`).
fn juzgar(j: &Juez, c: &Cuenta, por_que: &mut Texto) -> bool {
    use core::fmt::Write;
    por_que.vaciar();
    let mut sep = "";
    let mut di = |t: core::fmt::Arguments| {
        let _ = write!(por_que, "{sep}{t}");
        sep = ", ";
    };
    if c.colgado {
        di(format_args!("se colgo"));
    } else {
        match c.salio {
            Some(0) => {}
            Some(n) => di(format_args!("salio con {n}")),
            None => di(format_args!("no dijo con que salio")),
        }
    }
    if c.mal > 0 {
        di(format_args!("{} MAL", c.mal));
    }
    let vale_otra = j.otra.is_some_and(|(b, n)| c.bien == b && c.nota == n);
    if !vale_otra {
        if j.bien >= 0 && c.bien != j.bien {
            di(format_args!("{} bien (pide {})", c.bien, j.bien));
        }
        if j.notas >= 0 && c.nota != j.notas {
            di(format_args!("{} nota (pide {})", c.nota, j.notas));
        }
    }
    sep.is_empty()
}

/// El informe al disco, de una llamada.
fn guardar(informe: &Texto) -> bool {
    let Ok(f) = bmo::Archivo::create(RUTA_INFORME) else { return false };
    let escritos = f.escribir_de(&informe._bloque, 0, informe.n as u64);
    escritos as usize == informe.n && f.close()
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    use core::fmt::Write;
    let mut arg = [0u8; 200];
    let n = bmo::argumentos(&mut arg);
    let pedido = core::str::from_utf8(&arg[..n]).unwrap_or("");
    let mut tope_s = TOPE_S;
    for p in pedido.split_whitespace() {
        if let Some(t) = p.strip_prefix("tope=").and_then(|t| t.parse().ok()) {
            tope_s = t;
        }
    }
    // Lo que se pidio: sin nada (o solo el tope), todos.
    let nombres = || pedido.split_whitespace().filter(|p| !p.starts_with("tope="));
    let todos = nombres().next().is_none();
    let quiero = |j: &Juez| todos || nombres().any(|p| p == j.nombre || p == j.grupo);

    let (Some(mut informe), Some(mut salida), Some(mut por_que)) = (Texto::nuevo(INFORME_BYTES), Texto::nuevo(SALIDA_BYTES), Texto::nuevo(1024)) else {
        di!("JUECES: sin memoria\n");
        bmo::salir();
    };
    let hz = bmo::info(bmo::INFO_TSC_HZ).max(1);

    // La tabla: los grupos y los jueces que se piden.
    let mut grupo = "";
    let mut total = 0u32;
    for l in TABLA.lines() {
        let t = l.trim();
        if let Some(g) = t.strip_prefix('[').and_then(|g| g.strip_suffix(']')) {
            grupo = g;
        } else if !t.is_empty() && !t.starts_with('#') {
            total += leer_juez(t, grupo).is_some_and(|j| quiero(&j)) as u32;
        }
    }
    if total == 0 {
        di!("JUECES: ninguno se llama asi (los grupos: dentro, tandas, d3d12)\n");
        bmo::salir();
    }
    di!("JUECES: {total} jueces de PROTON-X, uno tras otro (tope {tope_s} s cada uno); el informe, en informe/jueces.txt\n");
    let _ = writeln!(informe, "Los jueces de PROTON-X en BMO-X: {total}, tope {tope_s} s cada uno\n");

    let (mut buenos, mut malos, mut k) = (0u32, 0u32, 0u32);
    let empezo = ahora_s(hz);
    grupo = "";
    for l in TABLA.lines() {
        let t = l.trim();
        if let Some(g) = t.strip_prefix('[').and_then(|g| g.strip_suffix(']')) {
            grupo = g;
            continue;
        }
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        let Some(j) = leer_juez(t, grupo).filter(|j| quiero(j)) else { continue };
        k += 1;
        let t0 = ahora_s(hz);
        let c = correr(&j, tope_s, hz, &mut salida);
        let s = ahora_s(hz).saturating_sub(t0);
        let bien = juzgar(&j, &c, &mut por_que);
        let razon = core::str::from_utf8(por_que.texto()).unwrap_or("?");
        let avisos = if c.avisos > 0 { "; con avisos" } else { "" };
        if bien {
            buenos += 1;
            di!("[{k}/{total}] {:<12} bien      {} bien, sale con 0 ({s} s{avisos})\n", j.nombre, c.bien);
            let _ = writeln!(informe, "{:<12} bien      {} bien, sale con 0 ({s} s)", j.nombre, c.bien);
        } else {
            malos += 1;
            di!("[{k}/{total}] {:<12} DISTINTO  {razon} ({s} s)\n", j.nombre);
            let _ = writeln!(informe, "{:<12} DISTINTO  {razon} ({s} s)", j.nombre);
        }
        if !j.que.is_empty() {
            let _ = writeln!(informe, "               [{}]", j.que);
        }
        // Lo que hay que leer: lo MAL, las notas y los avisos; y si salio
        // DISTINTO, lo que dijo entero.
        for linea in salida.texto().split(|&x| x == b'\n') {
            if !bien || se_apunta(linea) {
                informe.poner(b"               ");
                informe.poner(linea);
                informe.poner(b"\n");
                if !bien && se_apunta(linea) {
                    di!("               {}\n", core::str::from_utf8(linea).unwrap_or("?"));
                }
            }
        }
    }
    let minutos = ahora_s(hz).saturating_sub(empezo) / 60;
    di!("\nJUECES: {buenos} dicen lo que pide la tabla, {malos} distintos, en {minutos} min\n");
    let _ = writeln!(informe, "\n{buenos} dicen lo que pide la tabla, {malos} distintos, en {minutos} min");
    if guardar(&informe) {
        di!("JUECES: el informe entero, en informe/jueces.txt\n");
    } else {
        di!("JUECES: no se pudo escribir informe/jueces.txt (lo de arriba es todo)\n");
    }
    bmo::salir();
}

#[panic_handler]
fn panico(info: &core::panic::PanicInfo) -> ! {
    bmo::consola("JUECES: panico\n");
    if let Some(s) = info.message().as_str() {
        bmo::consola(s);
        bmo::consola("\n");
    }
    bmo::salir();
}
