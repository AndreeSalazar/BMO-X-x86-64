//! **THE EXPLORER'S TREE** -- the package as it IS on disk (every folder and
//! every file, not only the ones a `mod` names), in the order the OWNER chose.
//!
//! ```text
//!    fill      walks the package folder through a `Lister` (in F1: the
//!              kernel's Directorio object, one per process -- never the
//!              cursor of F12), breadth first: ONE folder open at a time
//!    apply     reads the owner's arrangement from Titan.toml
//!    layout    the rows to draw: depth first, folded folders closed
//!    place     one item before or after a sibling: the new order
//!    settings  Titan.toml rewritten with that arrangement
//! ```
//!
//! ** NOT A TO Z, ON PURPOSE (the owner, 04-10: "organizar a mi manera ... sin
//! pelear con orden de A hasta la Z"). Inside a folder, what the owner placed
//! goes first, in the order placed; what nobody placed yet follows in the
//! order of the disk -- the order things were made in, so a new file appears
//! at the end of its folder, where the eye expects the newest.
//!
//! The arrangement lives in `Titan.toml`, next to `[layout]` (13.5 of
//! TITAN_MAESTRO: the main node holds the view), as TOML a person can read
//! and edit:
//!
//! ```toml
//!    [explorer]                         # folder = its children, in order
//!    "." = ["src", "Titan.toml", "LEEME.txt"]
//!    "src" = ["main.titan", "ship.titan"]
//!
//!    [explorer.folded]                  # the folders kept closed
//!    "src/physics" = true
//! ```
//!
//! [layer] PURE like the rest of this crate. And PLAIN: every field is an
//! integer (no `bool`, no enum), so any bit pattern is a valid `Tree` -- F1
//! keeps it in a borrowed block, not on its 64 KiB stack, and calls `clear`.

use crate::text::{commas, is_name, lines, quoted, trim, Path};

pub const MAX_ITEMS: usize = 192;
/// Where every name of the tree is kept, one after the other.
pub const NAMES: usize = 4096;
/// How deep the walk goes below the package folder.
pub const MAX_DEPTH: usize = 8;
/// The longest name the explorer makes or accepts.
pub const NAME_MAX: usize = 48;

/// Whoever can say what a folder holds. `put(name, is_folder, bytes)` once
/// per entry; `false` if the folder cannot be opened.
pub trait Lister {
    fn list(&mut self, folder: &[u8], put: &mut dyn FnMut(&[u8], bool, u64)) -> bool;
}

/// One entry. Integers only (see the header).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Item {
    /// The parent's index + 1; 0 is the package folder itself.
    parent: u16,
    at: u16,
    len: u8,
    /// 1 = a folder.
    folder: u8,
    /// 1 = kept closed.
    folded: u8,
    /// Its place among its siblings as the owner set it, from 1; 0 = not
    /// placed (it goes after the placed ones, in disk order).
    rank: u16,
    /// Its place among its siblings on disk.
    disk: u16,
    size: u32,
}

impl Item {
    #[cfg(test)]
    const EMPTY: Item = Item { parent: 0, at: 0, len: 0, folder: 0, folded: 0, rank: 0, disk: 0, size: 0 };
}

pub struct Tree {
    items: [Item; MAX_ITEMS],
    n: u16,
    names: [u8; NAMES],
    used: u16,
    rows: [u16; MAX_ITEMS],
    depths: [u8; MAX_ITEMS],
    n_rows: u16,
    /// 1 = something did not fit (items, names or depth): the tree is SHORT
    /// and says so instead of pretending.
    cut: u8,
}

/// Why a name is refused, in words for the column.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NameError {
    Empty,
    TooLong,
    /// Only letters, digits, `_`, `-`, `.` and inner spaces.
    BadChar,
    /// `.` and `..` are not names.
    Dots,
    /// A `.titan` is called like its module: lowercase, digits and `_`.
    BadModule,
    /// A sibling already has it.
    Taken,
}

impl NameError {
    pub fn say(self) -> &'static str {
        match self {
            NameError::Empty => "el nombre esta vacio",
            NameError::TooLong => "el nombre es demasiado largo (48 como mucho)",
            NameError::BadChar => "solo letras, cifras, _ - . y espacios por dentro",
            NameError::Dots => "`.` y `..` no son nombres",
            NameError::BadModule => "un .titan se llama como su modulo: minusculas, cifras y _",
            NameError::Taken => "en esa carpeta ya hay algo con ese nombre",
        }
    }
}

/// Is `name` a name the explorer can create or rename to? (Not whether it is
/// free: that is `Tree::free`.)
pub fn check_name(name: &[u8]) -> Result<(), NameError> {
    if name.is_empty() {
        return Err(NameError::Empty);
    }
    if name.len() > NAME_MAX {
        return Err(NameError::TooLong);
    }
    if name == b"." || name == b".." {
        return Err(NameError::Dots);
    }
    let ok = |c: u8| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-' | b'.' | b' ');
    if !name.iter().all(|&c| ok(c)) || name[0] == b' ' || name[name.len() - 1] == b' ' {
        return Err(NameError::BadChar);
    }
    if let Some(stem) = name.strip_suffix(b".titan") {
        if !is_name(stem) {
            return Err(NameError::BadModule);
        }
    }
    Ok(())
}

/// Why an item cannot go where it was dropped.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PlaceError {
    Same,
    /// Reordering is among siblings; moving between folders is another gesture.
    OtherFolder,
    NoSuch,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SettingsError {
    /// The new Titan.toml does not fit in the buffer.
    DoesNotFit,
}

/// The key that orders siblings: placed first by rank, then by disk, and the
/// index last so two keys are never equal.
fn key(it: &Item, i: usize) -> (u8, u16, u16, u16) {
    (if it.rank == 0 { 1 } else { 0 }, it.rank, it.disk, i as u16)
}

impl Tree {
    /// Empties it. Enough on ANY bytes: every field is an integer, and only
    /// the first `n` items are ever read.
    pub fn clear(&mut self) {
        self.n = 0;
        self.used = 0;
        self.n_rows = 0;
        self.cut = 0;
    }

    pub fn len(&self) -> usize {
        self.n as usize
    }

    pub fn is_empty(&self) -> bool {
        self.n == 0
    }

    /// `true` if something did not fit and the tree is short.
    pub fn cut(&self) -> bool {
        self.cut != 0
    }

    fn item(&self, i: usize) -> Option<&Item> {
        self.items[..self.n as usize].get(i)
    }

    pub fn name(&self, i: usize) -> &[u8] {
        match self.item(i) {
            Some(it) => &self.names[it.at as usize..it.at as usize + it.len as usize],
            None => b"",
        }
    }

    pub fn is_folder(&self, i: usize) -> bool {
        self.item(i).is_some_and(|it| it.folder != 0)
    }

    pub fn is_folded(&self, i: usize) -> bool {
        self.item(i).is_some_and(|it| it.folded != 0)
    }

    pub fn size(&self, i: usize) -> u32 {
        self.item(i).map_or(0, |it| it.size)
    }

    /// The folder `i` is in: `None` for the package folder.
    pub fn parent(&self, i: usize) -> Option<usize> {
        self.item(i).and_then(|it| (it.parent as usize).checked_sub(1))
    }

    /// The folder new things go into when `i` is selected: `i` itself if it is
    /// a folder, else the folder it is in. `None` = the package folder.
    pub fn folder_for(&self, i: Option<usize>) -> Option<usize> {
        let i = i?;
        if self.is_folder(i) {
            Some(i)
        } else {
            self.parent(i)
        }
    }

    fn push(&mut self, parent: u16, name: &[u8], folder: bool, size: u64, disk: u16) -> bool {
        let (n, used) = (self.n as usize, self.used as usize);
        if n >= MAX_ITEMS || used + name.len() > NAMES || name.is_empty() || name.len() > NAME_MAX {
            self.cut = 1;
            return false;
        }
        self.names[used..used + name.len()].copy_from_slice(name);
        self.items[n] = Item {
            parent,
            at: used as u16,
            len: name.len() as u8,
            folder: folder as u8,
            folded: 0,
            rank: 0,
            disk,
            size: size.min(u32::MAX as u64) as u32,
        };
        self.n += 1;
        self.used += name.len() as u16;
        true
    }

    /// The package folder `root` (`titan/asteroids`), walked whole.
    ///
    /// ** Breadth first, so the Lister has ONE folder open at a time: in F1
    /// each is a kernel Directorio, and there are eight for the whole system.
    pub fn fill<L: Lister>(&mut self, l: &mut L, root: &[u8]) {
        self.clear();
        // `next` = the next item to open as a folder; the package folder
        // itself goes first (code 0).
        let mut next = 0usize;
        let mut first = true;
        loop {
            let (code, path) = if first {
                first = false;
                (0u16, Path::new(&[root]))
            } else {
                while next < self.n as usize && self.items[next].folder == 0 {
                    next += 1;
                }
                if next >= self.n as usize {
                    break;
                }
                let i = next;
                next += 1;
                if self.depth(i) + 1 >= MAX_DEPTH {
                    self.cut = 1;
                    continue;
                }
                (i as u16 + 1, self.full_path(root, i))
            };
            let Some(path) = path else {
                self.cut = 1;
                continue;
            };
            let mut disk = 0u16;
            let me = &mut *self;
            l.list(path.as_bytes(), &mut |name, folder, size| {
                if me.push(code, name, folder, size, disk) {
                    disk += 1;
                }
            });
        }
        self.layout();
    }

    /// Steps below the package folder: 0 for what is right in it.
    pub fn depth(&self, i: usize) -> usize {
        let mut d = 0;
        let mut at = self.parent(i);
        while let Some(p) = at {
            d += 1;
            if d > MAX_DEPTH {
                break;
            }
            at = self.parent(p);
        }
        d
    }

    /// Where `i` is, from the package folder: `src/physics/collide.titan`.
    pub fn path_of(&self, i: usize) -> Option<Path> {
        self.item(i)?;
        let mut chain = [0usize; MAX_DEPTH + 1];
        let mut k = 0;
        let mut at = Some(i);
        while let Some(x) = at {
            if k >= chain.len() {
                return None;
            }
            chain[k] = x;
            k += 1;
            at = self.parent(x);
        }
        let mut p = Path::EMPTY;
        for (j, &x) in chain[..k].iter().rev().enumerate() {
            if j > 0 {
                p.push(b"/")?;
            }
            p.push(self.name(x))?;
        }
        Some(p)
    }

    /// The same, from the root of the volume: `titan/asteroids/src/...`.
    pub fn full_path(&self, root: &[u8], i: usize) -> Option<Path> {
        let rel = self.path_of(i)?;
        Path::new(&[root, b"/", rel.as_bytes()])
    }

    /// The folder `parent` (None = the package folder) and a child's name.
    pub fn child(&self, parent: Option<usize>, name: &[u8]) -> Option<usize> {
        let code = parent.map_or(0, |p| p as u16 + 1);
        (0..self.n as usize).find(|&i| self.items[i].parent == code && self.name(i) == name)
    }

    /// The item at `rel` (`src/ship.titan`); `"."` or `""` is not an item.
    pub fn find(&self, rel: &[u8]) -> Option<usize> {
        let mut at = None;
        for step in rel.split(|&c| c == b'/') {
            if step.is_empty() {
                return None;
            }
            at = Some(self.child(at, step)?);
        }
        at
    }

    /// Is `name` free inside `folder`? `except` is the item being renamed.
    pub fn free(&self, folder: Option<usize>, name: &[u8], except: Option<usize>) -> Result<(), NameError> {
        match self.child(folder, name) {
            Some(i) if Some(i) != except => Err(NameError::Taken),
            _ => Ok(()),
        }
    }

    /// The owner's arrangement, from the text of Titan.toml. What it names and
    /// is not on disk any more is ignored: the disk is the truth.
    pub fn apply(&mut self, manifest: &[u8]) {
        #[derive(PartialEq)]
        enum S {
            Order,
            Folded,
            Other,
        }
        let mut s = S::Other;
        for (_, line) in lines(manifest) {
            if line.is_empty() || line.starts_with(b"#") {
                continue;
            }
            if let Some(head) = line.strip_prefix(b"[").and_then(|l| l.strip_suffix(b"]")) {
                s = match trim(head) {
                    b"explorer" => S::Order,
                    b"explorer.folded" => S::Folded,
                    _ => S::Other,
                };
                continue;
            }
            if s == S::Other {
                continue;
            }
            let Some(eq) = line.iter().position(|&c| c == b'=') else { continue };
            let Some(folder) = quoted(trim(&line[..eq])) else { continue };
            let value = trim(&line[eq + 1..]);
            let target = if folder == b"." { Some(None) } else { self.find(folder).filter(|&i| self.is_folder(i)).map(Some) };
            let Some(target) = target else { continue };
            match s {
                S::Order => {
                    let Some(list) = value.strip_prefix(b"[").and_then(|v| v.strip_suffix(b"]")) else { continue };
                    let mut rank = 0u16;
                    for name in commas(list).filter_map(quoted) {
                        if let Some(c) = self.child(target, name) {
                            rank += 1;
                            self.items[c].rank = rank;
                        }
                    }
                }
                S::Folded => {
                    if let (Some(i), b"true") = (target, value) {
                        self.items[i].folded = 1;
                    }
                }
                S::Other => {}
            }
        }
        self.layout();
    }

    /// The children of `folder` in the order they are shown.
    fn children(&self, folder: u16) -> impl Iterator<Item = usize> + '_ {
        let mut last: Option<(u8, u16, u16, u16)> = None;
        core::iter::from_fn(move || {
            let best = (0..self.n as usize)
                .filter(|&i| self.items[i].parent == folder)
                .map(|i| (key(&self.items[i], i), i))
                .filter(|(k, _)| last.map_or(true, |l| *k > l))
                .min_by_key(|(k, _)| *k)?;
            last = Some(best.0);
            Some(best.1)
        })
    }

    /// The rows: depth first, in the owner's order; inside a folded folder,
    /// nothing. Call after anything changes.
    pub fn layout(&mut self) {
        self.n_rows = 0;
        // (folder code, last child shown) per level: no recursion, no arrays
        // of children -- the next child is found by its key.
        let mut stack = [(0u16, usize::MAX); MAX_DEPTH + 1];
        let mut top = 1usize;
        while top > 0 {
            let (folder, after) = stack[top - 1];
            let next = {
                let mut it = self.children(folder);
                if after == usize::MAX {
                    it.next()
                } else {
                    let mut found = false;
                    let mut out = None;
                    for c in it {
                        if found {
                            out = Some(c);
                            break;
                        }
                        found = c == after;
                    }
                    out
                }
            };
            let Some(c) = next else {
                top -= 1;
                continue;
            };
            stack[top - 1].1 = c;
            let r = self.n_rows as usize;
            if r < MAX_ITEMS {
                self.rows[r] = c as u16;
                self.depths[r] = (top - 1) as u8;
                self.n_rows += 1;
            }
            if self.items[c].folder != 0 && self.items[c].folded == 0 && top < stack.len() {
                stack[top] = (c as u16 + 1, usize::MAX);
                top += 1;
            }
        }
    }

    /// The rows to draw: `(item, depth)`.
    pub fn rows(&self) -> impl Iterator<Item = (usize, u8)> + '_ {
        (0..self.n_rows as usize).map(|r| (self.rows[r] as usize, self.depths[r]))
    }

    pub fn row_count(&self) -> usize {
        self.n_rows as usize
    }

    /// Opens a closed folder, closes an open one.
    pub fn toggle(&mut self, i: usize) {
        if self.is_folder(i) {
            self.items[i].folded ^= 1;
            self.layout();
        }
    }

    /// `moving` goes right before (or after) `target`, its sibling. Every
    /// sibling gets its rank from here on: the folder is now arranged by hand.
    pub fn place(&mut self, moving: usize, target: usize, after: bool) -> Result<(), PlaceError> {
        if moving == target {
            return Err(PlaceError::Same);
        }
        let (a, b) = (self.item(moving).ok_or(PlaceError::NoSuch)?, self.item(target).ok_or(PlaceError::NoSuch)?);
        if a.parent != b.parent {
            return Err(PlaceError::OtherFolder);
        }
        let folder = a.parent;
        let mut rank = 0u16;
        let mut give = |items: &mut [Item; MAX_ITEMS], i: usize| {
            rank += 1;
            items[i].rank = rank;
        };
        // The order as shown now, without `moving`, and `moving` dropped in.
        let order: [u16; MAX_ITEMS] = {
            let mut o = [u16::MAX; MAX_ITEMS];
            for (k, c) in self.children(folder).enumerate() {
                o[k] = c as u16;
            }
            o
        };
        for &c in order.iter().take_while(|&&c| c != u16::MAX) {
            let c = c as usize;
            if c == moving {
                continue;
            }
            if c == target && !after {
                give(&mut self.items, moving);
            }
            give(&mut self.items, c);
            if c == target && after {
                give(&mut self.items, moving);
            }
        }
        self.layout();
        Ok(())
    }

    /// `i` now has `name` (it was renamed on disk): its place and its folds
    /// stay, because they hang from the item and not from the old name.
    pub fn rename(&mut self, i: usize, name: &[u8]) -> bool {
        let used = self.used as usize;
        if self.item(i).is_none() || used + name.len() > NAMES || name.is_empty() || name.len() > NAME_MAX {
            return false;
        }
        self.names[used..used + name.len()].copy_from_slice(name);
        self.items[i].at = used as u16;
        self.items[i].len = name.len() as u8;
        self.used += name.len() as u16;
        true
    }

    /// `manifest` with its `[explorer]` and `[explorer.folded]` replaced by
    /// the arrangement of this tree. Every other line goes out as it came in.
    pub fn settings(&self, manifest: &[u8], out: &mut [u8]) -> Result<usize, SettingsError> {
        let mut o = Out { buf: out, n: 0 };
        let mut skipping = false;
        for raw in manifest.split_inclusive(|&c| c == b'\n') {
            let t = trim(raw);
            if let Some(head) = t.strip_prefix(b"[").and_then(|l| l.strip_suffix(b"]")) {
                skipping = matches!(trim(head), b"explorer" | b"explorer.folded");
            }
            if !skipping {
                o.put(raw)?;
            }
        }
        // Trailing blank lines of the kept part go; one blank line, then ours.
        while o.n > 0 && o.buf[o.n - 1] == b'\n' && (o.n < 2 || o.buf[o.n - 2] == b'\n') {
            o.n -= 1;
        }
        if o.n > 0 && o.buf[o.n - 1] != b'\n' {
            o.put(b"\n")?;
        }
        let arranged = |code: u16| (0..self.n as usize).any(|i| self.items[i].parent == code && self.items[i].rank != 0);
        let folders = core::iter::once(0u16).chain((0..self.n as usize).filter(|&i| self.items[i].folder != 0).map(|i| i as u16 + 1));
        let mut header = false;
        for code in folders.filter(|&c| arranged(c)) {
            if !header {
                o.put(b"\n[explorer]\n")?;
                header = true;
            }
            o.put(b"\"")?;
            match code {
                0 => o.put(b".")?,
                c => o.put(self.path_of(c as usize - 1).ok_or(SettingsError::DoesNotFit)?.as_bytes())?,
            }
            o.put(b"\" = [")?;
            for (k, c) in self.children(code).enumerate() {
                if k > 0 {
                    o.put(b", ")?;
                }
                o.put(b"\"")?;
                o.put(self.name(c))?;
                o.put(b"\"")?;
            }
            o.put(b"]\n")?;
        }
        let mut header = false;
        for i in (0..self.n as usize).filter(|&i| self.items[i].folder != 0 && self.items[i].folded != 0) {
            if !header {
                o.put(b"\n[explorer.folded]\n")?;
                header = true;
            }
            o.put(b"\"")?;
            o.put(self.path_of(i).ok_or(SettingsError::DoesNotFit)?.as_bytes())?;
            o.put(b"\" = true\n")?;
        }
        Ok(o.n)
    }
}

struct Out<'a> {
    buf: &'a mut [u8],
    n: usize,
}

impl Out<'_> {
    fn put(&mut self, s: &[u8]) -> Result<(), SettingsError> {
        let end = self.n + s.len();
        self.buf.get_mut(self.n..end).ok_or(SettingsError::DoesNotFit)?.copy_from_slice(s);
        self.n = end;
        Ok(())
    }
}

/// An empty tree on the heap, for the tests of this crate (F1 keeps it in a
/// borrowed block: it is ~8 KiB).
#[cfg(test)]
pub(crate) fn boxed() -> std::boxed::Box<Tree> {
    std::boxed::Box::new(Tree {
        items: [Item::EMPTY; MAX_ITEMS],
        n: 0,
        names: [0; NAMES],
        used: 0,
        rows: [0; MAX_ITEMS],
        depths: [0; MAX_ITEMS],
        n_rows: 0,
        cut: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::boxed::Box;
    use std::string::String;
    use std::vec::Vec;

    /// A disk in memory: `(folder, [(name, is_folder)])`, in disk order.
    struct Disk(Vec<(&'static str, Vec<(&'static str, bool)>)>);

    impl Lister for Disk {
        fn list(&mut self, folder: &[u8], put: &mut dyn FnMut(&[u8], bool, u64)) -> bool {
            match self.0.iter().find(|(f, _)| f.as_bytes() == folder) {
                Some((_, entries)) => {
                    for (n, d) in entries {
                        put(n.as_bytes(), *d, 7);
                    }
                    true
                }
                None => false,
            }
        }
    }

    fn disk() -> Disk {
        Disk(std::vec![
            ("titan/ast", std::vec![("Titan.toml", false), ("src", true), ("LEEME.txt", false)]),
            ("titan/ast/src", std::vec![("main.titan", false), ("ship.titan", false), ("physics", true), ("physics.titan", false)]),
            ("titan/ast/src/physics", std::vec![("collide.titan", false)]),
        ])
    }

    fn tree() -> Box<Tree> {
        let mut t = boxed();
        t.fill(&mut disk(), b"titan/ast");
        t
    }

    fn shown(t: &Tree) -> String {
        t.rows()
            .map(|(i, d)| {
                let mut s = String::from("  ").repeat(d as usize);
                s.push_str(core::str::from_utf8(t.name(i)).unwrap());
                if t.is_folder(i) {
                    s.push_str(if t.is_folded(i) { " +" } else { " /" });
                }
                s
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn the_whole_package_in_disk_order_not_a_to_z() {
        let t = tree();
        assert_eq!(
            shown(&t),
            "Titan.toml\nsrc /\n  main.titan\n  ship.titan\n  physics /\n    collide.titan\n  physics.titan\nLEEME.txt"
        );
        assert!(!t.cut());
        assert_eq!(t.path_of(t.find(b"src/physics/collide.titan").unwrap()).unwrap().as_bytes(), b"src/physics/collide.titan");
        assert_eq!(t.full_path(b"titan/ast", t.find(b"src").unwrap()).unwrap().as_bytes(), b"titan/ast/src");
    }

    #[test]
    fn the_owner_s_order_and_folds_come_from_titan_toml() {
        let mut t = tree();
        t.apply(b"[package]\nname = \"ast\"\n\n[explorer]\n\".\" = [\"LEEME.txt\", \"src\"]\n\"src\" = [\"physics.titan\", \"gone.titan\", \"main.titan\"]\n\n[explorer.folded]\n\"src/physics\" = true\n");
        assert_eq!(
            shown(&t),
            "LEEME.txt\nsrc /\n  physics.titan\n  main.titan\n  ship.titan\n  physics +\nTitan.toml"
        );
    }

    #[test]
    fn placing_by_hand_and_writing_it_back_reads_the_same() {
        let mut t = tree();
        let ship = t.find(b"src/ship.titan").unwrap();
        let main = t.find(b"src/main.titan").unwrap();
        t.place(ship, main, false).unwrap();
        t.toggle(t.find(b"src/physics").unwrap());
        assert!(shown(&t).starts_with("Titan.toml\nsrc /\n  ship.titan\n  main.titan\n  physics +"), "{}", shown(&t));
        assert_eq!(t.place(ship, t.find(b"LEEME.txt").unwrap(), true), Err(PlaceError::OtherFolder));

        let manifest = b"[package]\nname = \"ast\"\n\n[layout]\nmain = [1, 2]\n\n[explorer]\n\"src\" = [\"old\"]\n";
        let mut out = [0u8; 1024];
        let n = t.settings(manifest, &mut out).unwrap();
        let text = core::str::from_utf8(&out[..n]).unwrap();
        assert_eq!(
            text,
            "[package]\nname = \"ast\"\n\n[layout]\nmain = [1, 2]\n\n[explorer]\n\"src\" = [\"ship.titan\", \"main.titan\", \"physics\", \"physics.titan\"]\n\n[explorer.folded]\n\"src/physics\" = true\n"
        );
        // The manifest reader of this crate still reads it.
        assert!(crate::manifest::parse(&out[..n]).is_ok());
        // And a fresh tree with it shows the same.
        let mut u = tree();
        u.apply(&out[..n]);
        assert_eq!(shown(&u), shown(&t));
    }

    #[test]
    fn a_renamed_item_keeps_its_place() {
        let mut t = tree();
        let ship = t.find(b"src/ship.titan").unwrap();
        t.place(ship, t.find(b"src/main.titan").unwrap(), false).unwrap();
        assert!(t.rename(ship, b"nave.titan"));
        assert!(shown(&t).contains("src /\n  nave.titan\n  main.titan"), "{}", shown(&t));
        assert_eq!(t.free(t.parent(ship), b"main.titan", None), Err(NameError::Taken));
        assert_eq!(t.free(t.parent(ship), b"nave.titan", Some(ship)), Ok(()));
    }

    #[test]
    fn names_the_explorer_accepts() {
        for ok in [&b"rock.titan"[..], b"LEEME.txt", b"mis notas.md", b"assets", b"a-b_c.2"] {
            assert_eq!(check_name(ok), Ok(()), "{:?}", core::str::from_utf8(ok));
        }
        assert_eq!(check_name(b""), Err(NameError::Empty));
        assert_eq!(check_name(b".."), Err(NameError::Dots));
        assert_eq!(check_name(b"a/b"), Err(NameError::BadChar));
        assert_eq!(check_name(b"a\"b"), Err(NameError::BadChar));
        assert_eq!(check_name(b" a"), Err(NameError::BadChar));
        assert_eq!(check_name(b"Rock.titan"), Err(NameError::BadModule));
        assert_eq!(check_name(&[b'a'; 49]), Err(NameError::TooLong));
    }

    #[test]
    fn a_tree_that_does_not_fit_says_it_is_short() {
        let mut big = std::vec![];
        let names: Vec<String> = (0..MAX_ITEMS + 5).map(|i| std::format!("f{}", i)).collect();
        let leaked: &'static Vec<String> = Box::leak(Box::new(names));
        for n in leaked.iter() {
            big.push((n.as_str(), false));
        }
        let mut t = tree();
        t.fill(&mut Disk(std::vec![("p", big)]), b"p");
        assert_eq!(t.len(), MAX_ITEMS);
        assert!(t.cut());
    }
}
