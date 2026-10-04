//! `paquete` -- A PACKAGE: several `.titan`, ONE program (level 9).
//!
//! TITAN_MAESTRO 4.3 and U3. A file is ONE module; its header says who it is,
//! its children and whom it talks to:
//!
//! ```text
//!    mod physics "mueve los cuerpos"     who it is (the first line)
//!    use ship                            whom it talks to
//!    mod collide                         its children: physics/collide.titan
//!    mod rules in "reglas/juego.titan"   ... or wherever the PARENT says
//! ```
//!
//! The files are found the way titan-lector finds them for F1 (`header.rs`,
//! `package.rs`, `text::default_place`): from the root, following `mod`, never
//! listing a folder. So the graph F1 draws and the program this compiles are
//! the same tree.
//!
//! ** WHAT A MODULE MAY REACH, and the NO of each (U3: "el compilador lo
//! compara con lo que de verdad llama"):
//!
//! ```text
//!    ship.avanza()   only into a CHILD (`mod ship`) or a `use ship`     T0080
//!                    and only what is `pub` there                       T0082
//!    use ship        and then nothing of ship is used                   T0081
//!    mod ship        and ship.titan is not there, or says another name  T0083
//!    use a -> use b -> use a, or a child that uses its parent           T0084
//!                    (the layers only go DOWN: law L8, in the compiler)
//! ```
//!
//! `use gpu` and `use director` are the two nodes of BMO-X that F1 draws
//! (the 3060 and the DIRECTOR): there is nothing in them to call yet, so they
//! are declarations, and the permission behind them is the manifest's (U2).
//!
//! ** ONE PROGRAM AFTER THIS. Every item gets its whole name -- `avanza` of
//! `ship` is `ship.avanza`, the root's keep theirs -- and the modules are
//! joined into one `Program`. Names, the checker and the calculation then
//! work as they did with one file: they never learn there were several.
//!
//! ** WHERE A NO IS. Each file's lines are counted from where the previous
//! file's end (`base`), like the global positions of rustc's source map: a
//! line names ONE place in the whole package. `locate` turns it back into
//! the file and its own line, the message's text included.

use crate::check::distance;
use crate::message::{Code, Message};
use bmo_titan_contrato::{Permission, Permissions};
use crate::tree::{Arm, Expr, Program, Step, Stmt, Ty};

/// The two nodes of BMO-X a header may `use` (F1 draws them), and the
/// permission of the `Titan.toml` each one needs -- the same pairs as
/// titan-lector's `package.rs` (U2).
pub const SYSTEM: [(&str, Permission); 2] = [("gpu", Permission::Gpu), ("director", Permission::Screen)];

/// More files than this is a `mod` that goes round, not a package.
const MAX_FILES: usize = 64;

/// One file of the package.
pub struct File {
    /// Its path from the package: `src/physics.titan`.
    pub path: String,
    pub src: String,
    /// Where its line 1 is, in the package's count.
    base: usize,
    program: Program,
}

/// The files, read and parsed: the root first, then each one under the
/// module that says `mod` of it.
pub struct Package {
    pub files: Vec<File>,
    /// What its `Titan.toml` asks for (U2). A package without one asks for
    /// nothing; a file alone is such a package.
    pub permissions: Permissions,
}

impl Package {
    /// The file a package-wide line is in, and its own line there.
    fn place(&self, line: usize) -> Option<(usize, usize)> {
        let k = self.files.iter().rposition(|f| f.base < line || (f.base == 0 && line == 0))?;
        Some((k, line - self.files[k].base))
    }

    /// The message with its file and its own line -- and every "linea N" in
    /// its text turned back the same way, naming the file when it is another.
    pub fn locate(&self, mut m: Message) -> Message {
        let Some((k, line)) = self.place(m.line) else { return m };
        m.line = line;
        m.file = Some(self.files[k].path.clone());
        for text in [&mut m.what, &mut m.why, &mut m.how] {
            *text = self.lines_in(text, k);
        }
        m
    }

    fn lines_in(&self, text: &str, here: usize) -> String {
        const WORD: &str = "linea ";
        let mut out = String::new();
        let mut rest = text;
        while let Some(i) = rest.find(WORD) {
            out += &rest[..i + WORD.len()];
            rest = &rest[i + WORD.len()..];
            let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
            match rest[..digits].parse::<usize>().ok().and_then(|n| self.place(n)) {
                Some((k, line)) if k == here => out += &line.to_string(),
                Some((k, line)) => out += &format!("{} de {}", line, self.files[k].path),
                None => out += &rest[..digits],
            }
            rest = &rest[digits..];
        }
        out + rest
    }
}

/// Where a child lives when its `mod` does not say: next to the root for the
/// root's children, in a folder named after the parent for anyone else's --
/// `text::default_place` of titan-lector, the same rule.
fn default_place(parent: &str, root: bool, child: &str) -> String {
    if root {
        match parent.rfind('/') {
            Some(i) => format!("{}/{}.titan", &parent[..i], child),
            None => format!("{}.titan", child),
        }
    } else {
        format!("{}/{}.titan", parent.strip_suffix(".titan").unwrap_or(parent), child)
    }
}

/// Reads the package from its root file, following every `mod`. `read` gives
/// a file by its path from the package, or `None` if it is not there.
pub fn load(root: &str, src: &str, read: &mut dyn FnMut(&str) -> Option<String>) -> Result<Package, Message> {
    let mut pkg = Package { files: Vec::new(), permissions: Permissions::NONE };
    // ** THE MANIFEST (U2, level 11 G0): a package is `Titan.toml` + `src/`,
    // as F1 reads it; only a root in `src/` has one next to its folder.
    if root.starts_with("src/") {
        if let Some(text) = read("Titan.toml") {
            pkg.permissions = manifest(&text)?;
        }
    }
    let mut next_base = 0;
    let mut add = |pkg: &mut Package, path: String, src: String| -> Result<usize, Message> {
        let base = next_base;
        // One line more than it has: a NO at its end ("el final del
        // fichero") is still in it, never in the next.
        next_base += src.lines().count() + 1;
        let program = parse_at(&src, base).map_err(|mut m| {
            // Not in the package yet: its own lines, said by hand.
            m.line = m.line.saturating_sub(base);
            m.file = Some(path.clone());
            m
        })?;
        pkg.files.push(File { path, src, base, program });
        Ok(pkg.files.len() - 1)
    };
    add(&mut pkg, root.to_string(), src.to_string())?;
    let mut i = 0;
    while i < pkg.files.len() {
        let children = pkg.files[i].program.children.clone();
        for c in children {
            let parent = &pkg.files[i].path;
            let path = c.path.clone().unwrap_or_else(|| default_place(parent, i == 0, &c.name));
            if let Some(other) = pkg.files.iter().find(|f| f.program.module == c.name) {
                let m = Message::new(
                    Code::Twice,
                    c.line,
                    c.col,
                    &format!("el modulo `{}` esta dos veces en el paquete", c.name),
                    &format!("ya es {}: un `use {}` no sabria a cual ir", other.path, c.name),
                    "cambia el nombre de uno de los dos (y su `mod`)",
                );
                return Err(pkg.locate(m));
            }
            let Some(text) = read(&path) else {
                let m = Message::new(
                    Code::NoModule,
                    c.line,
                    c.col,
                    &format!("`mod {}` y no hay fichero {}", c.name, path),
                    "un hijo vive en su propio fichero: junto a main.titan los de main, y en la carpeta de su padre los demas (como cargo)",
                    &format!("crea {} con su cabecera: mod {} \"que hace\", o di donde esta: mod {} in \"carpeta/{}.titan\"", path, c.name, c.name, c.name),
                );
                return Err(pkg.locate(m));
            };
            if pkg.files.len() >= MAX_FILES {
                let m = Message::new(Code::NoModule, c.line, c.col, &format!("el paquete pasa de {} ficheros", MAX_FILES), "un `mod` que vuelve a un fichero ya leido no termina nunca", "revisa las rutas de los `mod ... in`");
                return Err(pkg.locate(m));
            }
            let k = add(&mut pkg, path.clone(), text)?;
            let said = pkg.files[k].program.module.clone();
            if said != c.name {
                let m = Message::new(
                    Code::NoModule,
                    c.line,
                    c.col,
                    &format!("`mod {}` lleva a {}, y ese fichero dice `mod {}`", c.name, path, said),
                    "el nombre del hijo y la primera linea de su fichero son el mismo: es lo que F1 dibuja y lo que un `use` nombra",
                    &format!("o `mod {}` aqui, o `mod {} \"...\"` en la primera linea de {}", said, c.name, path),
                );
                return Err(pkg.locate(m));
            }
        }
        i += 1;
    }
    Ok(pkg)
}

/// The permissions a `Titan.toml` asks for, read by titan-lector's own
/// parser: the manifest F1 shows and the one the compiler obeys are one.
fn manifest(text: &str) -> Result<Permissions, Message> {
    use bmo_titan_lector::manifest::{parse, ManifestError};
    parse(text.as_bytes()).map(|m| m.permissions).map_err(|e| {
        let (line, what) = match e {
            ManifestError::NoName => (1, "`[package]` no dice su `name`, o no es un nombre del lenguaje".to_string()),
            ManifestError::BadLine(n) => (n, format!("la linea {} no es `[seccion]`, `clave = valor` ni un comentario `#`", n)),
            ManifestError::TooManyPositions => (1, "`[layout]` tiene mas posiciones que nodos caben en el grafo".to_string()),
        };
        let mut m = Message::new(
            Code::BadManifest,
            line,
            1,
            &format!("el Titan.toml no se entiende: {}", what),
            "el manifiesto dice que es el paquete y que PIDE (U2); uno que no se lee no pide nada con certeza, y nada se adivina",
            "[package]
             name = \"mi_app\"
             [permissions]
             gpu = \"compute\"",
        );
        m.file = Some("Titan.toml".to_string());
        m
    })
}

/// One file, its lines counted from `base`.
fn parse_at(src: &str, base: usize) -> Result<Program, Message> {
    let mut tokens = crate::lex::lex(src).map_err(|mut m| {
        m.line += base;
        m
    })?;
    for t in &mut tokens {
        t.line += base;
    }
    crate::parse::parse(&tokens)
}

/// What one module has, by its OWN names.
struct Items {
    fns: Vec<(String, bool)>,
    types: Vec<(String, bool)>,
    enums: Vec<(String, bool)>,
    /// A case, and whether its enum is `pub`.
    cases: Vec<(String, bool)>,
    traits: Vec<(String, bool)>,
    /// A fn a trait promises, and whether the trait is `pub` (level 10).
    methods: Vec<(String, bool)>,
}

fn items(p: &Program) -> Items {
    Items {
        fns: p.functions.iter().map(|f| (f.name.clone(), f.public)).collect(),
        types: p.types.iter().map(|t| (t.name.clone(), t.public)).collect(),
        enums: p.enums.iter().map(|e| (e.name.clone(), e.public)).collect(),
        cases: p.enums.iter().flat_map(|e| e.cases.iter().map(move |c| (c.name.clone(), e.public))).collect(),
        traits: p.traits.iter().map(|t| (t.name.clone(), t.public)).collect(),
        methods: p.traits.iter().flat_map(|t| t.methods.iter().map(move |m| (m.name.clone(), t.public))).collect(),
    }
}

/// What a qualified name must be, by where it is written.
#[derive(Clone, Copy, PartialEq)]
enum Want {
    /// `x.y(...)`: a function, or a case that carries values.
    Call,
    /// `x.Y { ... }`: a type.
    Record,
    /// `n: x.Y`: a type or an enum.
    Type,
    /// `x.Y` alone, or an arm of a `match`: a case.
    Case,
    /// `trait x.Forma for T`: a trait (level 10).
    Trait,
}

struct Resolver<'p> {
    names: Vec<String>,
    items: Vec<Items>,
    /// Who each module may reach: its children and its `use`s.
    reach: Vec<Vec<usize>>,
    /// The `use`s that were used: (module, used).
    used: Vec<(usize, usize)>,
    pkg: &'p Package,
}

impl Resolver<'_> {
    fn prefix(&self, m: usize) -> String {
        if m == 0 {
            String::new()
        } else {
            format!("{}.", self.names[m])
        }
    }

    fn own(&self, m: usize, name: &str, want: Want) -> bool {
        let it = &self.items[m];
        let has = |l: &[(String, bool)]| l.iter().any(|x| x.0 == name);
        match want {
            Want::Call => has(&it.fns) || has(&it.cases) || has(&it.methods),
            Want::Record => has(&it.types),
            Want::Type => has(&it.types) || has(&it.enums) || has(&it.traits),
            Want::Case => has(&it.cases),
            Want::Trait => has(&it.traits),
        }
    }

    /// The whole name of `name`, written in module `m`.
    fn resolve(&mut self, m: usize, name: &str, want: Want, at: (usize, usize)) -> Result<String, Message> {
        let Some((x, y)) = name.split_once('.') else {
            return Ok(if self.own(m, name, want) { format!("{}{}", self.prefix(m), name) } else { name.to_string() });
        };
        let Some(j) = self.names.iter().position(|n| n == x) else {
            let near = self.names.iter().map(String::as_str).min_by_key(|k| distance(k, x)).filter(|k| distance(k, x) <= 2);
            return Err(Message::new(
                Code::Unknown,
                at.0,
                at.1,
                &format!("no hay un modulo `{}` en este paquete", x),
                &format!("`{}.{}` busca `{}` dentro de un modulo, y los modulos son: {}", x, y, y, self.names.join(", ")),
                &match near {
                    Some(k) => format!("quisiste decir `{}.{}`?", k, y),
                    None => format!("declaralo: mod {} en la cabecera, y su fichero {}.titan", x, x),
                },
            ));
        };
        if j == m {
            return Ok(format!("{}{}", self.prefix(m), y));
        }
        if !self.reach[m].contains(&j) {
            return Err(Message::new(
                Code::Undeclared,
                at.0,
                at.1,
                &format!("`{}` usa `{}` y su cabecera no lo dice", self.names[m], x),
                "un modulo dice ARRIBA con quien habla (U3): quien lee la cabecera sabe de quien depende sin leer el cuerpo, y F1 dibuja ese cable",
                &format!("agrega a la cabecera de {}: use {}", self.pkg.files[m].path, x),
            ));
        }
        let it = &self.items[j];
        let find = |l: &[(String, bool)]| l.iter().find(|e| e.0 == y).map(|e| e.1);
        let public = match want {
            Want::Call => find(&it.fns).or(find(&it.cases)).or(find(&it.methods)),
            Want::Record => find(&it.types),
            Want::Type => find(&it.types).or(find(&it.enums)).or(find(&it.traits)),
            Want::Case => find(&it.cases),
            Want::Trait => find(&it.traits),
        };
        let Some(public) = public else {
            let all: Vec<&str> = [&it.fns, &it.types, &it.enums, &it.cases, &it.traits, &it.methods].iter().flat_map(|l| l.iter().map(|e| e.0.as_str())).collect();
            let near = all.iter().copied().min_by_key(|k| distance(k, y)).filter(|k| distance(k, y) <= 2);
            return Err(Message::new(
                Code::Unknown,
                at.0,
                at.1,
                &format!("`{}` no tiene `{}`", x, y),
                &format!("lo que {} define es: {}", self.pkg.files[j].path, if all.is_empty() { "nada todavia".to_string() } else { all.join(", ") }),
                &match near {
                    Some(k) => format!("quisiste decir `{}.{}`?", x, k),
                    None => format!("definelo en {}, con `pub` delante", self.pkg.files[j].path),
                },
            ));
        };
        if !public {
            return Err(Message::new(
                Code::Private,
                at.0,
                at.1,
                &format!("`{}` es de `{}` y no es `pub`", y, x),
                "lo que un modulo no marca `pub` es suyo: puede cambiarlo sin romper a nadie. Lo que se ve desde fuera se DICE",
                &format!("si debe verse desde fuera, en {}: pub fn {} / pub type / pub enum", self.pkg.files[j].path, y),
            ));
        }
        if !self.used.contains(&(m, j)) {
            self.used.push((m, j));
        }
        Ok(format!("{}{}", self.prefix(j), y))
    }

    fn ty(&mut self, m: usize, t: &mut Ty, at: (usize, usize)) -> Result<(), Message> {
        match t {
            Ty::Named(n) => *n = self.resolve(m, n, Want::Type, at)?,
            Ty::Table(inner, _) => self.ty(m, inner, at)?,
            _ => {}
        }
        Ok(())
    }

    fn body(&mut self, m: usize, body: &mut [Stmt]) -> Result<(), Message> {
        for st in body {
            match st {
                Stmt::Call(c) => {
                    c.callee = self.resolve(m, &c.callee, Want::Call, (c.line, c.col))?;
                    for a in &mut c.args {
                        self.expr(m, a)?;
                    }
                }
                Stmt::Let(l) | Stmt::Set(l) => {
                    if let Some(t) = &mut l.ty {
                        self.ty(m, t, (l.line, l.col))?;
                    }
                    self.expr(m, &mut l.value)?;
                }
                Stmt::If(i) => {
                    self.expr(m, &mut i.cond)?;
                    self.body(m, &mut i.then)?;
                    self.body(m, &mut i.other)?;
                }
                Stmt::While(w) => {
                    self.expr(m, &mut w.cond)?;
                    self.body(m, &mut w.body)?;
                }
                Stmt::For(f) => {
                    self.expr(m, &mut f.from)?;
                    self.expr(m, &mut f.to)?;
                    if let Some(o) = &mut f.over {
                        self.expr(m, o)?;
                    }
                    self.body(m, &mut f.body)?;
                }
                Stmt::Return { value: Some(v), .. } => self.expr(m, v)?,
                Stmt::Return { .. } | Stmt::Break { .. } | Stmt::Continue { .. } => {}
                Stmt::SetAt { path, value, .. } => {
                    for s in path {
                        if let Step::Index(i) = s {
                            self.expr(m, i)?;
                        }
                    }
                    self.expr(m, value)?;
                }
                Stmt::Match { value, arms, .. } => {
                    self.expr(m, value)?;
                    for Arm { case, body, line, col, .. } in arms {
                        *case = self.resolve(m, case, Want::Case, (*line, *col))?;
                        self.body(m, body)?;
                    }
                }
            }
        }
        Ok(())
    }

    fn expr(&mut self, m: usize, e: &mut Expr) -> Result<(), Message> {
        match e {
            Expr::Call { callee, args, line, col } => {
                *callee = self.resolve(m, callee, Want::Call, (*line, *col))?;
                for a in args {
                    self.expr(m, a)?;
                }
            }
            Expr::Name { name, line, col } => {
                if self.own(m, name, Want::Case) {
                    *name = self.resolve(m, name, Want::Case, (*line, *col))?;
                }
            }
            // `forma.Nada`: a case of another module, written bare. `forma`
            // is a module: no value may take a module's name (`names`).
            Expr::Field { base, name, line, col } if matches!(&**base, Expr::Name { name: x, .. } if self.names[1..].contains(x)) => {
                let Expr::Name { name: x, .. } = &**base else { unreachable!("the guard") };
                let whole = self.resolve(m, &format!("{}.{}", x, name), Want::Case, (*line, *col))?;
                *e = Expr::Name { name: whole, line: *line, col: *col };
            }
            Expr::Record { name, fields, line, col } => {
                *name = self.resolve(m, name, Want::Record, (*line, *col))?;
                for (_, v) in fields {
                    self.expr(m, v)?;
                }
            }
            Expr::Bin { left, right, .. } | Expr::Index { base: left, index: right, .. } => {
                self.expr(m, left)?;
                self.expr(m, right)?;
            }
            Expr::Neg { value, .. } | Expr::Not { value, .. } | Expr::Round { value, .. } | Expr::Repeat { item: value, .. } | Expr::Field { base: value, .. } => self.expr(m, value)?,
            Expr::Table { items, .. } => {
                for i in items {
                    self.expr(m, i)?;
                }
            }
            Expr::Int { .. } | Expr::Text { .. } | Expr::Bool { .. } | Expr::Dec { .. } | Expr::Lend { .. } => {}
        }
        Ok(())
    }
}

/// Every name a value takes in a body: (name, line, col).
fn values_named(body: &[Stmt], out: &mut Vec<(String, usize, usize)>) {
    for st in body {
        match st {
            Stmt::Let(l) => out.push((l.name.clone(), l.line, l.col)),
            Stmt::For(f) => {
                out.push((f.var.clone(), f.var_at.0, f.var_at.1));
                values_named(&f.body, out);
            }
            Stmt::If(i) => {
                values_named(&i.then, out);
                values_named(&i.other, out);
            }
            Stmt::While(w) => values_named(&w.body, out),
            Stmt::Match { arms, .. } => {
                for a in arms {
                    out.extend(a.binds.iter().cloned());
                    values_named(&a.body, out);
                }
            }
            _ => {}
        }
    }
}

/// The package as ONE program: its modules checked against each other, then
/// joined with their whole names. A NO comes back located.
pub fn join(pkg: &Package) -> Result<Program, Message> {
    join_inner(pkg).map_err(|m| pkg.locate(m))
}

fn join_inner(pkg: &Package) -> Result<Program, Message> {
    let names: Vec<String> = pkg.files.iter().map(|f| f.program.module.clone()).collect();
    let mut reach: Vec<Vec<usize>> = vec![Vec::new(); names.len()];
    for (i, f) in pkg.files.iter().enumerate() {
        for c in &f.program.children {
            let j = names.iter().position(|n| n == &c.name).expect("load: every child was read");
            reach[i].push(j);
        }
        for (k, u) in f.program.uses.iter().enumerate() {
            if f.program.uses[..k].iter().any(|v| v.name == u.name) {
                return Err(Message::new(Code::Twice, u.line, u.col, &format!("`use {}` esta dos veces", u.name), "un cable, una vez", "deja uno"));
            }
            if let Some((_, need)) = SYSTEM.iter().find(|s| s.0 == u.name) {
                // ** U2 in the compiler: talking to the 3060 or to the screen
                // needs the manifest to ASK for it -- the rule F1 already
                // draws (`NoPermission` of titan-lector), now a NO here too.
                if !pkg.permissions.allows(*need) {
                    return Err(Message::new(
                        Code::NoPermission,
                        u.line,
                        u.col,
                        &format!("`use {}` y el Titan.toml no pide `{}`", u.name, need.key()),
                        "lo que un programa usa de BMO-X lo PIDE su manifiesto (U2): el kernel lo concede o no, y el certificado del .bex dice desde que linea",
                        &format!("pidelo en el Titan.toml del paquete:
             [permissions]
             {} = true", need.key()),
                    ));
                }
                continue;
            }
            match names.iter().position(|n| n == &u.name) {
                Some(j) if j == i => {
                    return Err(Message::new(Code::Cycle, u.line, u.col, &format!("`{}` se usa a si mismo", u.name), "lo suyo lo llama sin `use`: avanza(), no ship.avanza()", "quita este `use`"));
                }
                Some(j) => reach[i].push(j),
                None => {
                    let known: Vec<&str> = names.iter().map(String::as_str).chain(SYSTEM.iter().map(|s| s.0)).collect();
                    let near = known.iter().copied().min_by_key(|k| distance(k, &u.name)).filter(|k| distance(k, &u.name) <= 2);
                    return Err(Message::new(
                        Code::Unknown,
                        u.line,
                        u.col,
                        &format!("no hay un modulo `{}` en este paquete", u.name),
                        &format!("un `use` nombra un modulo del paquete ({}) o un nodo de BMO-X (gpu, director)", names.join(", ")),
                        &match near {
                            Some(k) => format!("quisiste decir `{}`?", k),
                            None => format!("si es nuevo: mod {} en la cabecera de su padre, y su fichero", u.name),
                        },
                    ));
                }
            }
        }
    }
    cycle(pkg, &names, &reach)?;
    let mut r = Resolver { items: pkg.files.iter().map(|f| items(&f.program)).collect(), names, reach, used: Vec::new(), pkg };
    let mut out = pkg.files[0].program.clone();
    out.functions.clear();
    out.types.clear();
    out.enums.clear();
    out.traits.clear();
    out.impls.clear();
    for (m, f) in pkg.files.iter().enumerate() {
        let mut p = f.program.clone();
        {
            // ** One name, one thing, across the package too: a value named
            // like a module, or (in a module) like one of its own functions
            // or cases, would make `x` two things to whoever reads it.
            let mut named = Vec::new();
            for g in p.functions.iter().chain(p.impls.iter().flat_map(|i| &i.functions)) {
                named.extend(g.params.iter().map(|a| (a.name.clone(), a.line, a.col)));
                values_named(&g.body, &mut named);
            }
            for (n, line, col) in named {
                // The root's own name is not a module anyone reaches.
                let clash = if r.names[1..].contains(&n) {
                    Some(format!("el modulo `{}`", n))
                } else if m > 0 && r.own(m, &n, Want::Call) {
                    Some(format!("algo de `{}` (una fn o un caso)", r.names[m]))
                } else {
                    None
                };
                if let Some(what) = clash {
                    return Err(Message::new(Code::Taken, line, col, &format!("`{}` ya es {}", n, what), &format!("un nombre dice UNA cosa: quien lee `{}` tiene que saber cual es", n), &format!("llama al valor de otra forma: {}_valor", n)));
                }
            }
        }
        let prefix = r.prefix(m);
        for g in &mut p.functions {
            for a in &mut g.params {
                r.ty(m, &mut a.ty, (a.line, a.col))?;
            }
            if let Some(t) = &mut g.ret {
                r.ty(m, t, (g.line, g.col))?;
            }
            r.body(m, &mut g.body)?;
            g.name = format!("{}{}", prefix, g.name);
        }
        for t in &mut p.types {
            for fl in &mut t.fields {
                r.ty(m, &mut fl.ty, (fl.line, fl.col))?;
            }
            t.name = format!("{}{}", prefix, t.name);
        }
        for e in &mut p.enums {
            for c in &mut e.cases {
                for t in &mut c.fields {
                    r.ty(m, t, (c.line, c.col))?;
                }
                c.name = format!("{}{}", prefix, c.name);
            }
            e.name = format!("{}{}", prefix, e.name);
        }
        // Level 10: a trait's name and its fn get the whole name, like a fn;
        // a `trait ... for` names its trait and its type from where it is,
        // and keeps its fn's short names -- they say WHICH fn of the trait.
        for t in &mut p.traits {
            for s in &mut t.methods {
                for a in &mut s.params {
                    r.ty(m, &mut a.ty, (a.line, a.col))?;
                }
                if let Some(rt) = &mut s.ret {
                    r.ty(m, rt, (s.line, s.col))?;
                }
                s.name = format!("{}{}", prefix, s.name);
            }
            t.name = format!("{}{}", prefix, t.name);
        }
        for i in &mut p.impls {
            i.trait_name = r.resolve(m, &i.trait_name, Want::Trait, (i.line, i.col))?;
            r.ty(m, &mut i.ty, (i.line, i.col))?;
            for g in &mut i.functions {
                for a in &mut g.params {
                    r.ty(m, &mut a.ty, (a.line, a.col))?;
                }
                if let Some(t) = &mut g.ret {
                    r.ty(m, t, (g.line, g.col))?;
                }
                r.body(m, &mut g.body)?;
            }
        }
        out.functions.extend(p.functions);
        out.types.extend(p.types);
        out.enums.extend(p.enums);
        out.traits.extend(p.traits);
        out.impls.extend(p.impls);
    }
    // ** A `use` that nothing uses: the header says a connection the body
    // does not have (U3, the other half). F1 would draw a cable to nowhere.
    for (i, f) in pkg.files.iter().enumerate() {
        for u in &f.program.uses {
            let Some(j) = r.names.iter().position(|n| n == &u.name) else { continue };
            if !r.used.contains(&(i, j)) {
                return Err(Message::new(
                    Code::Unused,
                    u.line,
                    u.col,
                    &format!("`use {}` y `{}` no usa nada de `{}`", u.name, r.names[i], u.name),
                    "la cabecera dice con quien habla un modulo, y el compilador lo compara con lo que de verdad llama (U3): un cable que no lleva nada es una dependencia que no existe",
                    &format!("llama a algo suyo (`{}.algo()`), o quita el `use`", u.name),
                ));
            }
        }
    }
    Ok(out)
}

/// ** THE LAYERS ONLY GO DOWN (law L8, in the compiler). A parent depends on
/// its children (`mod`) and a module on what it `use`s; a way back to where
/// it started is a cycle: T0084, with the way written.
fn cycle(pkg: &Package, names: &[String], reach: &[Vec<usize>]) -> Result<(), Message> {
    fn walk(at: usize, reach: &[Vec<usize>], state: &mut [u8], path: &mut Vec<usize>) -> Option<Vec<usize>> {
        state[at] = 1;
        path.push(at);
        for &to in &reach[at] {
            if state[to] == 1 {
                let from = path.iter().position(|&p| p == to).expect("on the path");
                let mut c = path[from..].to_vec();
                c.push(to);
                return Some(c);
            }
            if state[to] == 0 {
                if let Some(c) = walk(to, reach, state, path) {
                    return Some(c);
                }
            }
        }
        path.pop();
        state[at] = 2;
        None
    }
    let mut state = vec![0u8; names.len()];
    for start in 0..names.len() {
        if state[start] != 0 {
            continue;
        }
        if let Some(c) = walk(start, reach, &mut state, &mut Vec::new()) {
            // Point at the `use` that closes it: the last step that is a `use`.
            let (a, b) = c.windows(2).map(|w| (w[0], w[1])).rev().find(|&(a, b)| pkg.files[a].program.uses.iter().any(|u| u.name == names[b])).unwrap_or((c[c.len() - 2], c[c.len() - 1]));
            let at = pkg.files[a].program.uses.iter().find(|u| u.name == names[b]).map(|u| (u.line, u.col)).unwrap_or((pkg.files[a].base + 1, 1));
            let way: Vec<&str> = c.iter().map(|&k| names[k].as_str()).collect();
            return Err(Message::new(
                Code::Cycle,
                at.0,
                at.1,
                &format!("los modulos vuelven sobre si mismos: {}", way.join(" -> ")),
                "las capas solo BAJAN (la ley L8 de la casa, aqui la cumple el compilador): un padre depende de sus hijos y cada uno de lo que usa; un camino de vuelta haria de todo el paquete un solo bloque",
                &format!("saca lo que {} y {} comparten a un modulo de abajo, y que los dos lo usen", names[a], names[b]),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::message::{Code, Message};

    /// A package in memory: (path, text), the root first.
    fn lower(files: &[(&str, &str)]) -> Result<crate::ir::Module, Message> {
        let (root, src) = files[0];
        crate::lower_package(root, src, &mut |p| files.iter().find(|f| f.0 == p).map(|f| f.1.to_string()))
    }

    fn printed(files: &[(&str, &str)]) -> Vec<String> {
        let m = lower(files).unwrap_or_else(|e| panic!("{:?}", e));
        m.flat
            .unwrap()
            .iter()
            .filter_map(|op| match op {
                crate::ir::Op::Write { parts, .. } => Some(parts.iter().map(|p| if let crate::ir::Value::Text(t, _) = p { t.clone() } else { String::new() }).collect()),
                _ => None,
            })
            .collect()
    }

    const FORMA: (&str, &str) = ("src/forma.titan", "mod forma \"las formas\"\n\npub enum Forma\n    Circulo(dec)\n    Nada\n\npub type Punto\n    x: int\n");

    #[test]
    fn cases_records_and_types_reach_across_modules() {
        let main = "mod main \"x\"\nmod forma\n\nfn mide(f: forma.Forma) -> dec\n    match f\n        forma.Circulo(r)\n            return 3 * r * r\n        forma.Nada\n            return 0\n\nfn main()\n    let p = forma.Punto { x: 4 }\n    print(mide(forma.Circulo(2.0)), \" \", mide(forma.Nada), \" \", p, \" \", forma.Nada)\n";
        assert_eq!(printed(&[("src/main.titan", main), FORMA]), ["12.0 0 Punto { x: 4 } Nada"]);
    }

    /// ** A NO of the checker, in a CHILD file: its file and its own line,
    /// and the lines its text names, too.
    #[test]
    fn a_no_in_another_file_says_which_file_and_its_own_line() {
        let main = "mod main \"x\"\nmod nave\n\nfn main()\n    nave.avanza()\n";
        let nave = "mod nave \"x\"\n\npub fn avanza()\n    print(x)\n    let x = 1\n";
        let e = lower(&[("src/main.titan", main), ("src/nave.titan", nave)]).unwrap_err();
        assert_eq!((e.code, e.file.as_deref(), e.line), (Code::NoValue, Some("src/nave.titan"), 4));
        assert!(e.why.contains("linea 5"), "{}", e.why);
    }

    #[test]
    fn the_system_nodes_and_the_names_of_a_package() {
        // `use gpu` is a declaration: nothing to call in it yet -- and it
        // needs its Titan.toml to ask for the 3060 (U2).
        let gpu = "mod main \"x\"\nuse gpu\n\nfn main()\n    print(1)\n";
        let asks = lower(&[("src/main.titan", gpu), ("Titan.toml", "[package]\nname = \"x\"\n[permissions]\ngpu = \"compute\"\n")]).unwrap();
        assert!(asks.permissions.allows(bmo_titan_contrato::Permission::Gpu));
        assert_eq!(lower(&[("src/main.titan", gpu)]).unwrap_err().code, Code::NoPermission);
        let code = |files: &[(&str, &str)]| lower(files).unwrap_err().code;
        // A `use` of a module that is not there.
        assert_eq!(code(&[("src/main.titan", "mod main \"x\"\nuse naves\n\nfn main()\n    print(1)\n")]), Code::Unknown);
        // A value named like a module.
        assert_eq!(code(&[("src/main.titan", "mod main \"x\"\nmod forma\n\nfn main()\n    let forma = 1\n    print(forma, forma.Nada)\n"), FORMA]), Code::Taken);
        // Two modules with one name.
        let two = [("src/main.titan", "mod main \"x\"\nmod a, b\n\nfn main()\n    print(1)\n"), ("src/a.titan", "mod a \"x\"\nmod c\n"), ("src/b.titan", "mod b \"x\"\nmod c\n"), ("src/a/c.titan", "mod c \"x\"\n"), ("src/b/c.titan", "mod c \"x\"\n")];
        assert_eq!(code(&two), Code::Twice);
        // The header goes on top, and a file is ONE module.
        assert_eq!(code(&[("src/main.titan", "mod main \"x\"\n\nfn main()\n    print(1)\nuse gpu\n")]), Code::Expected);
        assert_eq!(code(&[("src/main.titan", "mod main \"x\"\nmod otro \"y\"\n\nfn main()\n    print(1)\n")]), Code::Expected);
        // A child where its parent says.
        let placed = [("src/main.titan", "mod main \"x\"\nmod reglas in \"juego/reglas.titan\"\n\nfn main()\n    reglas.di()\n"), ("juego/reglas.titan", "mod reglas \"x\"\n\npub fn di()\n    print(\"regla\")\n")];
        assert_eq!(printed(&placed), ["regla"]);
    }
}
