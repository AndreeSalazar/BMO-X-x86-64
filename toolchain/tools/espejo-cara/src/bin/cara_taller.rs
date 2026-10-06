//! **LA COLUMNA DEL TALLER (F1), pintada en el anfitrion** por SU codigo
//! (`#[path]` a `explorer.rs`, `view.rs`, `canvas.rs`...), sobre un ESTRATOS
//! en memoria: la semilla `asteroids` y unas cosas de mas (una carpeta, un
//! LEEME, un `.titan` sin declarar). Es la CAMARA de `PLAN_TALLER` 8.7, ahora
//! en el repositorio: lo que se ve aqui es lo que pinta el Ryzen.
//!
//! `cara-taller <carpeta>` deja `arbol.png`, `escribiendo.png`, `menu.png` y
//! `arrastre.png`, del ancho de la columna, y la ventana entera con cada
//! solapa: `grafo.png`, `cielo.png`, `elementos.png` y `guia.png`. Y la TAB de
//! los nodos maestros (`PLAN_TALLER` 8.15): `tab.png`, abierta con la ficha de
//! un nodo, y `tab_filtro.png`, escribiendo "match". Y la solapa ESTRATOS
//! (`PLAN_LA_BANDEJA` T2) con una historia de EJEMPLO: `estratos.png` y
//! `estratos_pregunta.png` (el primer ENTER, preguntando); y su GUIA, las
//! puertas del contrato (`PLAN_LAS_RAMAS` R1): `estratos_guia.png`. Y sus
//! RAMAS (R5), manejadas con las teclas de verdad: `estratos_ramas.png`,
//! `estratos_mezcla.png` y `estratos_roto.png` (el candado roto).

#[path = "../../../../../Ultra_userspace/apps/taller/src/canvas.rs"]
#[allow(dead_code)]
mod canvas;
#[path = "../../../../../Ultra_userspace/apps/taller/src/player.rs"]
#[allow(dead_code)]
mod player;
#[path = "../../../../../Ultra_userspace/apps/taller/src/view.rs"]
#[allow(dead_code)]
mod view;
#[path = "../../../../../Ultra_userspace/apps/taller/src/faults.rs"]
#[allow(dead_code)]
mod faults;
#[path = "../../../../../Ultra_userspace/apps/taller/src/explorer.rs"]
#[allow(dead_code)]
mod explorer;
#[path = "../../../../../Ultra_userspace/apps/taller/src/space.rs"]
#[allow(dead_code)]
mod space;
#[path = "../../../../../Ultra_userspace/apps/taller/src/astros.rs"]
#[allow(dead_code)]
mod astros;
#[path = "../../../../../Ultra_userspace/apps/taller/src/guia.rs"]
#[allow(dead_code)]
mod guia;
#[path = "../../../../../Ultra_userspace/apps/taller/src/aspecto.rs"]
#[allow(dead_code)]
mod aspecto;
#[path = "../../../../../Ultra_userspace/apps/taller/src/editor.rs"]
#[allow(dead_code)]
mod editor;
#[path = "../../../../../Ultra_userspace/apps/taller/src/iconos.rs"]
#[allow(dead_code)]
mod iconos;
#[path = "../../../../../Ultra_userspace/apps/taller/src/tab.rs"]
#[allow(dead_code)]
mod tab;
#[path = "../../../../../Ultra_userspace/apps/taller/src/strata_guide.rs"]
#[allow(dead_code)]
mod strata_guide;
#[path = "../../../../../Ultra_userspace/apps/taller/src/guia_estratos_gen.rs"]
#[allow(dead_code)]
mod guia_estratos_gen;
#[path = "../../../../../Ultra_userspace/apps/taller/src/strata.rs"]
#[allow(dead_code)]
mod strata;
#[path = "../../../../../Ultra_userspace/apps/taller/src/branches.rs"]
#[allow(dead_code)]
mod branches;
#[path = "../../../../../Ultra_userspace/apps/taller/src/tema_gen.rs"]
#[allow(dead_code)]
mod tema_gen;

/// Lo que `explorer.rs` lee de la tienda (la de verdad, `store.rs`, habla con
/// el kernel): los mismos campos y los mismos dos metodos.
mod store {
    use bmo_titan_contrato::{Line, Name, NodeId};
    use bmo_titan_lector::explorer::Tree;
    use bmo_titan_lector::hang::{self, HangError, Plan};
    use bmo_titan_lector::{Loaded, Path};

    #[allow(dead_code)]
    #[derive(Clone, Copy, PartialEq)]
    pub enum Origin {
        Estratos(u64),
        Memory(&'static str),
    }

    pub struct Store {
        pub origin: Origin,
        pub library: Vec<(Name, Path)>,
        pub chosen: usize,
        pub loaded: Loaded,
        pub note: Option<(Line, bool)>,
        pub detail: Option<Line>,
        pub tree: Option<&'static mut Tree>,
    }

    impl Store {
        pub fn packages(&self) -> &[(Name, Path)] {
            &self.library
        }
        pub fn plan(&self, child: NodeId, parent: NodeId) -> Result<Plan, HangError> {
            hang::plan(&self.loaded, child, parent)
        }
    }
}

use bmo_espejo_cara::{png, Imagen};

// Las teclas del DIRECTOR, como `main.rs` las nombra: `strata.rs` y
// `branches.rs` las piden a la raiz del crate.
#[allow(dead_code)]
pub(crate) const KEY_UP: u8 = 0x80;
#[allow(dead_code)]
pub(crate) const KEY_DOWN: u8 = 0x81;
#[allow(dead_code)]
pub(crate) const KEY_LEFT: u8 = 0x82;
#[allow(dead_code)]
pub(crate) const KEY_RIGHT: u8 = 0x83;

/// Un volumen de EJEMPLO para la solapa RAMAS (PLAN_LAS_RAMAS R5): tres
/// ramas, una mezcla con cuatro choques, y el candado que se puede romper.
struct Muestra {
    ramas: Vec<(&'static str, bool)>,
    choques: Vec<(&'static str, u8, u8)>,
    contado: bool,
    roto: bool,
}

impl branches::Volume for Muestra {
    fn branch(&mut self, i: usize, dst: &mut [u8]) -> Option<(usize, bool)> {
        let (n, actual) = *self.ramas.get(i)?;
        dst[..n.len()].copy_from_slice(n.as_bytes());
        Some((n.len(), actual))
    }
    fn create(&mut self, name: &[u8]) -> bool {
        self.ramas.push((String::from_utf8_lossy(name).into_owned().leak(), false));
        true
    }
    fn switch(&mut self, name: &[u8]) -> bool {
        for r in &mut self.ramas {
            r.1 = r.0.as_bytes() == name;
        }
        true
    }
    fn count(&mut self, _: &[u8]) -> Option<(u32, u32)> {
        self.contado = true;
        Some((self.choques.len() as u32, 7))
    }
    fn conflict(&mut self, i: usize, dst: &mut [u8]) -> Option<(usize, u8, u8)> {
        let (p, lados, e) = *self.choques.get(i).filter(|_| self.contado)?;
        dst[..p.len()].copy_from_slice(p.as_bytes());
        Some((p.len(), lados, e))
    }
    fn choose(&mut self, i: usize, pick: u8) -> bool {
        self.choques[i].2 = pick;
        true
    }
    fn merge(&mut self) -> bool {
        self.contado = false;
        true
    }
    fn lock(&mut self) -> (u64, u32, u32) {
        let elegidos = self.choques.iter().filter(|c| c.2 != 0).count() as u32;
        match (self.contado, self.roto) {
            (false, _) => (0, 0, 0),
            (true, false) => (1, self.choques.len() as u32, elegidos),
            (true, true) => (2, self.choques.len() as u32, elegidos),
        }
    }
}
use bmo_titan_contrato::{Line, Name};
use bmo_titan_lector::explorer::{Lister, Tree};
use bmo_titan_lector::{read_package, seed, Fetch, Path, Source};
use explorer::{Edit, EditKind, Menu, Ui};
use store::{Origin, Store};

/// ESTRATOS en memoria, en el orden en que se hizo cada cosa.
struct Mem(Vec<(String, Option<Vec<u8>>)>);

impl Source for Mem {
    fn fetch(&mut self, path: &[u8], buf: &mut [u8]) -> Fetch {
        match self.0.iter().find(|(p, _)| p.as_bytes() == path) {
            Some((_, Some(b))) if b.len() <= buf.len() => {
                buf[..b.len()].copy_from_slice(b);
                Fetch::Found(b.len())
            }
            Some((_, Some(_))) => Fetch::TooBig,
            _ => Fetch::Missing,
        }
    }
}

impl Lister for Mem {
    fn list(&mut self, folder: &[u8], put: &mut dyn FnMut(&[u8], bool, u64)) -> bool {
        for (p, b) in &self.0 {
            if let Some(rest) = p.as_bytes().strip_prefix(folder).and_then(|r| r.strip_prefix(b"/")) {
                if !rest.contains(&b'/') {
                    put(rest, b.is_none(), b.as_ref().map_or(0, |b| b.len() as u64));
                }
            }
        }
        true
    }
}

fn tree() -> &'static mut Tree {
    // Un `Tree` son enteros: cero es uno valido, y `clear` lo vacia.
    let layout = std::alloc::Layout::new::<Tree>();
    let t = unsafe { &mut *(std::alloc::alloc_zeroed(layout) as *mut Tree) };
    t.clear();
    t
}

fn guardar(ruta: &str, px: &[u32], w: usize, h: usize, ancho: usize) {
    let mut corte = Vec::with_capacity(ancho * h);
    for y in 0..h {
        corte.extend(px[y * w..y * w + ancho].iter().map(|p| p & 0x00FF_FFFF));
    }
    let im = Imagen { ancho, alto: h, px: corte };
    if let Err(e) = png::escribir(ruta, &im) {
        eprintln!("NO: {e}");
        std::process::exit(1);
    }
}

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    let mut disco = Mem(Vec::new());
    for f in seed::FOLDERS {
        disco.0.push((f.to_string(), None));
    }
    for (p, t) in seed::FILES {
        disco.0.push((p.to_string(), Some(t.as_bytes().to_vec())));
    }
    disco.0.push(("titan/asteroids/docs".into(), None));
    disco.0.push(("titan/asteroids/docs/LEEME.txt".into(), Some(b"hola".to_vec())));
    disco.0.push(("titan/asteroids/src/ufo.titan".into(), Some(b"mod ufo \"un ovni\"\n".to_vec())));
    disco.0.push(("titan/asteroids/notas.txt".into(), Some(b"ideas".to_vec())));

    let root = b"titan/asteroids";
    let mut buf = [0u8; 4096];
    let loaded = read_package(&mut disco, root, &mut buf);
    let t = tree();
    t.fill(&mut disco, root);
    // El orden del propietario: docs arriba, y physics plegada.
    t.apply(b"[explorer]\n\".\" = [\"docs\", \"src\", \"Titan.toml\"]\n\n[explorer.folded]\n\"src/physics\" = true\n");

    let mut store = Store {
        origin: Origin::Estratos(1234),
        library: vec![(Name::new("asteroids"), Path::new(&[root]).unwrap())],
        chosen: 0,
        loaded,
        note: None,
        detail: None,
        tree: Some(t),
    };
    let (w, h) = (1280usize, 760usize);
    let pinta = |store: &Store, ui: &Ui, drag: Option<(usize, i32, i32)>, nombre: &str| {
        let mut px = vec![0u32; w * h];
        let mut cv = canvas::Canvas::new(px.as_mut_ptr(), w as u32, h as u32);
        explorer::draw(&mut cv, store, ui, None, 0);
        if let Some((i, x, y)) = drag {
            let cam = view::Camera::fit(&store.loaded.graph, w as i32, h as i32);
            explorer::draw_drag_item(&mut cv, store, ui, &cam, i, x, y);
        }
        guardar(&format!("{out}/{nombre}.png"), &px, w, h, 420);
    };

    let ship = store.tree.as_deref().unwrap().find(b"src/ship.titan");
    let mut ui = Ui::new();
    ui.picked = ship;
    pinta(&store, &ui, None, "arbol");

    let src = store.tree.as_deref().unwrap().find(b"src");
    let mut e = Edit::new(EditKind::NewFile(src), b"");
    for &c in b"wing.titan" {
        e.push(c);
    }
    ui.edit = Some(e);
    pinta(&store, &ui, None, "escribiendo");

    ui.edit = None;
    ui.menu = Some(Menu { x: 120, y: 330, item: ship, code: true });
    pinta(&store, &ui, None, "menu");

    ui.menu = None;
    store.note = Some((Line::new("rock.titan -> roca.titan; vuelve 4 lo deshace"), true));
    store.detail = Some(Line::new("los use de otros ficheros siguen con el nombre viejo: PROBLEMAS"));
    // ship arrastrado a la mitad de arriba de main: va DELANTE de main. El
    // punto se BUSCA con `drop_at`, el mismo que decide al soltar.
    let main = store.tree.as_deref().unwrap().find(b"src/main.titan").unwrap();
    let cam = view::Camera::fit(&store.loaded.graph, w as i32, h as i32);
    let s = ship.unwrap();
    let y = (0..h as i32).find(|&y| explorer::drop_at(&store, &ui, &cam, s, 120, y) == Some(explorer::Drop::Before(main))).unwrap_or(0);
    pinta(&store, &ui, Some((s, 120, y)), "arrastre");
    // La ventana entera, con cada solapa: el mismo grafo, dos miradas.
    let cam = view::Camera::fit(&store.loaded.graph, w as i32, h as i32);
    let player = player::Player::new();
    let marks = faults::collect(&store.loaded, None);
    let ship_node = store.loaded.graph.find(b"ship");
    for (tab, nombre) in [(space::Tab::Sky, "cielo"), (space::Tab::Elements, "elementos"), (space::Tab::Guide, "guia"), (space::Tab::Graph, "grafo")] {
        let scene = view::Scene {
            graph: &store.loaded.graph,
            script: None,
            player: &player,
            cam: &cam,
            now_ms: 5000,
            selected: ship_node,
            origin: b"asteroids",
            sky: None,
            flow_ms: Some(5000),
            faults: &marks,
            files: store.loaded.files(),
            turn: 96,
        };
        let mut px = vec![0u32; w * h];
        let mut cv = canvas::Canvas::new(px.as_mut_ptr(), w as u32, h as u32);
        match tab {
            space::Tab::Graph => view::draw(&mut cv, &scene),
            t => space::draw(&mut cv, &scene, t),
        }
        space::tabs(&mut cv, tab);
        explorer::draw(&mut cv, &store, &ui, ship_node, 0);
        guardar(&format!("{out}/{nombre}.png"), &px, w, h, w);
    }
    // La solapa ESTRATOS (PLAN_LA_BANDEJA T2): una historia de EJEMPLO --
    // catorce versiones, tres con nombre -- con una elegida, y la misma
    // con la pregunta del primer ENTER puesta.
    let mut historia = strata::History::EMPTY;
    let nombres: [(usize, &str); 3] = [(2, "mundo sano"), (6, "antes del dragon"), (11, "base del mundo")];
    historia.absent = false;
    historia.n = 14;
    for i in 0..historia.n {
        let minuto = 58 - 4 * i as u8;
        let f = bmo_rtc::Fecha { anio: 2026, mes: 10, dia: 5, hora: 13, minuto, segundo: (7 * i as u8) % 60 };
        let v = &mut historia.versions[i];
        v.when = bmo_rtc::empaquetar(&f);
        v.who = [7, 12, 9][i % 3];
        if let Some((_, n)) = nombres.iter().find(|(k, _)| *k == i) {
            v.name[..n.len()].copy_from_slice(n.as_bytes());
            v.name_len = n.len();
        }
    }
    // La 4 es una MEZCLA (R5): tiene dos padres.
    historia.versions[4].merge = true;
    historia.picked = Some(6);
    for (nombre, pregunta) in [("estratos", false), ("estratos_pregunta", true)] {
        if pregunta {
            let _ = strata::enter(&mut historia, 5000);
        }
        let scene = view::Scene {
            graph: &store.loaded.graph,
            script: None,
            player: &player,
            cam: &cam,
            now_ms: 5000,
            selected: None,
            origin: b"asteroids",
            sky: None,
            flow_ms: None,
            faults: &marks,
            files: store.loaded.files(),
            turn: 96,
        };
        let mut px = vec![0u32; w * h];
        let mut cv = canvas::Canvas::new(px.as_mut_ptr(), w as u32, h as u32);
        strata::draw(&mut cv, &historia, None, 5000);
        view::title(&mut cv, &scene);
        space::tabs(&mut cv, space::Tab::Strata);
        explorer::draw(&mut cv, &store, &ui, None, 0);
        guardar(&format!("{out}/{nombre}.png"), &px, w, h, w);
        if pregunta {
            // Y su GUIA (PLAN_LAS_RAMAS R1): las puertas del contrato, con
            // `volver` elegida, que es la que la historia acaba de ofrecer.
            let volver = guia_estratos_gen::DOORS.iter().position(|d| d.door == "ES_GESTO_VOLVER").expect("el contrato tiene volver");
            let mut px = vec![0u32; w * h];
            let mut cv = canvas::Canvas::new(px.as_mut_ptr(), w as u32, h as u32);
            strata_guide::draw(&mut cv, volver, None);
            view::title(&mut cv, &scene);
            space::tabs(&mut cv, space::Tab::StrataGuide);
            explorer::draw(&mut cv, &store, &ui, None, 0);
            guardar(&format!("{out}/estratos_guia.png"), &px, w, h, w);
        }
    }
    // La solapa RAMAS (PLAN_LAS_RAMAS R5), manejada con las MISMAS teclas que
    // en el Ryzen: la lista; M sobre `pruebas` cuenta la mezcla y se eligen
    // dos choques; y el candado ROTO cuando otro programa escribio.
    let mut vol = Muestra {
        ramas: vec![("principal", true), ("pruebas", false), ("dragon-nuevo", false)],
        choques: vec![("mundo/mapa.titan", 3, 0), ("mundo/dragon.titan", 3, 0), ("notas.txt", 1, 0), ("src/ship.titan", 3, 0)],
        contado: false,
        roto: false,
    };
    let mut ramas = Box::new(branches::Branches::EMPTY);
    ramas.refresh(&mut vol);
    ramas.key(KEY_DOWN, 1000, &mut vol);
    let foto = |ramas: &branches::Branches, nombre: &str| {
        let scene = view::Scene {
            graph: &store.loaded.graph,
            script: None,
            player: &player,
            cam: &cam,
            now_ms: 5000,
            selected: None,
            origin: b"asteroids",
            sky: None,
            flow_ms: None,
            faults: &marks,
            files: store.loaded.files(),
            turn: 96,
        };
        let mut px = vec![0u32; w * h];
        let mut cv = canvas::Canvas::new(px.as_mut_ptr(), w as u32, h as u32);
        branches::draw(&mut cv, Some(ramas), None, 5000);
        view::title(&mut cv, &scene);
        space::tabs(&mut cv, space::Tab::Branches);
        explorer::draw(&mut cv, &store, &ui, None, 0);
        guardar(&format!("{out}/{nombre}.png"), &px, w, h, w);
    };
    foto(&ramas, "estratos_ramas");
    ramas.key(b'm', 1000, &mut vol);
    assert_eq!(ramas.nc, 4, "contar trae los cuatro choques");
    ramas.key(b'a', 1000, &mut vol);
    ramas.key(b'b', 1000, &mut vol);
    assert_eq!((vol.choques[0].2, vol.choques[1].2), (1, 2), "A y B llegan al volumen");
    ramas.key(b'\r', 1000, &mut vol);
    assert!(ramas.said.is_some_and(|(_, bien)| !bien), "con choques sin elegir, ENTER no mezcla");
    foto(&ramas, "estratos_mezcla");
    // Esc no pierde nada: M sobre la misma rama vuelve a lo elegido.
    ramas.key(0x1B, 1000, &mut vol);
    ramas.key(b'm', 1000, &mut vol);
    assert_eq!(ramas.conflicts[1].pick, 2, "volver no pierde lo elegido");
    vol.roto = true;
    ramas.refresh(&mut vol);
    foto(&ramas, "estratos_roto");
    // Y entero: se cuenta otra vez, se elige todo, y dos ENTER mezclan.
    vol.roto = false;
    ramas.key(b'\r', 1000, &mut vol);
    for _ in 0..4 {
        ramas.key(b'a', 1000, &mut vol);
    }
    ramas.key(b'\r', 1000, &mut vol);
    ramas.key(b'\r', 1100, &mut vol);
    assert!(!vol.contado && ramas.said.is_some_and(|(_, bien)| bien), "dos ENTER con todo elegido mezclan");

    // GRAFO con dos cables en la mano (UE5): uno que se puede soltar y uno
    // que cerraria un ciclo, rojo y con su motivo.
    for (nombre, from, to) in [("cable_bien", "rock", "ship"), ("cable_ciclo", "ship", "physics")] {
        let scene = view::Scene {
            graph: &store.loaded.graph,
            script: None,
            player: &player,
            cam: &cam,
            now_ms: 5000,
            selected: None,
            origin: b"asteroids",
            sky: None,
            flow_ms: None,
            faults: &marks,
            files: store.loaded.files(),
            turn: 96,
        };
        let mut px = vec![0u32; w * h];
        let mut cv = canvas::Canvas::new(px.as_mut_ptr(), w as u32, h as u32);
        view::draw(&mut cv, &scene);
        space::tabs(&mut cv, space::Tab::Graph);
        let g = &store.loaded.graph;
        let (a, b) = (g.find(from.as_bytes()).unwrap(), g.find(to.as_bytes()).unwrap());
        let (bx, by, bw, bh) = view::node_rect(&cam, g.node(b).unwrap());
        let (x, y) = (bx + bw / 2, by + bh / 2);
        let note = bmo_titan_lector::wire::plan(&store.loaded, a, b).map_err(|e| bmo_titan_lector::wire::note(Err(e), from.as_bytes(), to.as_bytes()).unwrap());
        let why = note.as_ref().map(|_| ()).map_err(|l| l.as_bytes());
        view::draw_wire(&mut cv, g, &cam, a, x, y, Some(why));
        explorer::draw(&mut cv, &store, &ui, None, 0);
        guardar(&format!("{out}/{nombre}.png"), &px, w, h, w);
    }
    // La TAB sobre el GRAFO: abierta con las flechas en `semaforo` (nivel 3),
    // y filtrando "match" (quedan los del nivel 8 y los que lo nombran).
    let (up, down) = (0x80, 0x81);
    let scene = view::Scene {
        graph: &store.loaded.graph,
        script: None,
        player: &player,
        cam: &cam,
        now_ms: 5000,
        selected: None,
        origin: b"asteroids",
        sky: None,
        flow_ms: Some(5000),
        faults: &marks,
        files: store.loaded.files(),
        turn: 96,
    };
    for (nombre, teclas) in [("tab", &[down; 8][..]), ("tab_filtro", &[b'm', b'a', b't', b'c', b'h', down, down][..])] {
        let mut paleta = tab::Palette::open((0, 0));
        for &k in teclas {
            paleta.key(k, up, down);
        }
        let mut px = vec![0u32; w * h];
        let mut cv = canvas::Canvas::new(px.as_mut_ptr(), w as u32, h as u32);
        view::draw(&mut cv, &scene);
        space::tabs(&mut cv, space::Tab::Graph);
        explorer::draw(&mut cv, &store, &ui, None, 0);
        tab::draw(&mut cv, &paleta, 5000);
        guardar(&format!("{out}/{nombre}.png"), &px, w, h, w);
    }
    // El clic derecho en un NODO del grafo: el menu de su fichero, con
    // `Editar codigo` arriba.
    {
        let scene = view::Scene {
            graph: &store.loaded.graph,
            script: None,
            player: &player,
            cam: &cam,
            now_ms: 5000,
            selected: store.loaded.graph.find(b"main"),
            origin: b"asteroids",
            sky: None,
            flow_ms: Some(5000),
            faults: &marks,
            files: store.loaded.files(),
            turn: 96,
        };
        let main_item = store.tree.as_deref().unwrap().find(b"src/main.titan");
        let mut ui2 = Ui::new();
        ui2.picked = main_item;
        ui2.menu = Some(Menu { x: 700, y: 250, item: main_item, code: true });
        let mut px = vec![0u32; w * h];
        let mut cv = canvas::Canvas::new(px.as_mut_ptr(), w as u32, h as u32);
        view::draw(&mut cv, &scene);
        space::tabs(&mut cv, space::Tab::Graph);
        explorer::draw(&mut cv, &store, &ui2, scene.selected, 0);
        guardar(&format!("{out}/menu_nodo.png"), &px, w, h, w);
    }
    // HOLA: el paquete mas chico, un nodo que imprime -- su impresora dice
    // lo que sale, "hola mundo". Y luego el EDITOR: doble clic en el nodo,
    // se escribe " desde BMO-X" dentro del texto, y al guardar el nodo dice
    // lo nuevo.
    {
        let hola = read_package(&mut disco, b"titan/hola", &mut buf);
        let cam = view::Camera::fit(&hola.graph, w as i32, h as i32);
        let marks = faults::collect(&hola, None);
        let scene = view::Scene {
            graph: &hola.graph,
            script: None,
            player: &player,
            cam: &cam,
            now_ms: 5000,
            selected: hola.graph.find(b"main"),
            origin: b"hola",
            sky: None,
            flow_ms: Some(5000),
            faults: &marks,
            files: hola.files(),
            turn: 96,
        };
        let mut px = vec![0u32; w * h];
        let mut cv = canvas::Canvas::new(px.as_mut_ptr(), w as u32, h as u32);
        view::draw(&mut cv, &scene);
        space::tabs(&mut cv, space::Tab::Graph);
        guardar(&format!("{out}/hola.png"), &px, w, h, w);

        let rel = Path::new(&[b"src/main.titan"]).unwrap();
        let src = seed::FILES.iter().find(|f| f.0 == "titan/hola/src/main.titan").unwrap().1.as_bytes();
        let mut text = vec![0u8; 16 * 1024];
        text[..src.len()].copy_from_slice(src);
        let mut ed = editor::Editor::open(rel, Name::new("main"), src.len());
        let k = &editor::KEYS;
        for key in [k.down, k.down, k.down, k.end, k.left, k.left] {
            ed.key(&mut text, key, 0);
        }
        for &ch in b" desde BMO-X" {
            ed.key(&mut text, ch, 0);
        }
        for (nombre, guardado) in [("editor", false), ("editor_guardado", true)] {
            if guardado {
                // lo que hace `store.save_text`: ESTRATOS toma la version y
                // el latido siguiente relee el paquete
                let slot = disco.0.iter_mut().find(|(p, _)| p == "titan/hola/src/main.titan").unwrap();
                slot.1 = Some(text[..ed.len].to_vec());
                ed.saved();
            }
            let hola = read_package(&mut disco, b"titan/hola", &mut buf);
            let marks = faults::collect(&hola, None);
            let cam = view::Camera { x: 300, y: -20, zoom: 1000 };
            let scene = view::Scene {
                graph: &hola.graph,
                script: None,
                player: &player,
                cam: &cam,
                now_ms: 5000,
                selected: hola.graph.find(b"main"),
                origin: b"hola",
                sky: None,
                flow_ms: Some(5000),
                faults: &marks,
                files: hola.files(),
                turn: 96,
            };
            let mut px = vec![0u32; w * h];
            let mut cv = canvas::Canvas::new(px.as_mut_ptr(), w as u32, h as u32);
            view::draw(&mut cv, &scene);
            space::tabs(&mut cv, space::Tab::Graph);
            editor::draw(&mut cv, &mut ed, &text, 0);
            guardar(&format!("{out}/{nombre}.png"), &px, w, h, w);
        }
    }
    println!("ok: {out}/hola.png editor.png editor_guardado.png menu_nodo.png");
    println!("ok: {out}/arbol.png escribiendo.png menu.png arrastre.png grafo.png cielo.png elementos.png guia.png estratos.png estratos_pregunta.png estratos_guia.png estratos_ramas.png estratos_mezcla.png estratos_roto.png tab.png tab_filtro.png");
}
