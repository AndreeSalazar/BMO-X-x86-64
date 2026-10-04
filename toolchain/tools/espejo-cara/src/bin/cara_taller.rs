//! **LA COLUMNA DEL TALLER (F1), pintada en el anfitrion** por SU codigo
//! (`#[path]` a `explorer.rs`, `view.rs`, `canvas.rs`...), sobre un ESTRATOS
//! en memoria: la semilla `asteroids` y unas cosas de mas (una carpeta, un
//! LEEME, un `.titan` sin declarar). Es la CAMARA de `PLAN_TALLER` 8.7, ahora
//! en el repositorio: lo que se ve aqui es lo que pinta el Ryzen.
//!
//! `cara-taller <carpeta>` deja `arbol.png`, `escribiendo.png`, `menu.png` y
//! `arrastre.png`, del ancho de la columna, y la ventana entera con cada
//! solapa: `grafo.png`, `cielo.png`, `elementos.png` y `guia.png`.

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
    ui.menu = Some(Menu { x: 120, y: 330, item: ship });
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
    println!("ok: {out}/arbol.png escribiendo.png menu.png arrastre.png grafo.png cielo.png elementos.png guia.png");
}
