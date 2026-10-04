//! **THE CERTIFICATE** -- what a TITAN++ program uses of BMO-X, and from which
//! line: the paper the two judges pass to each other (TITAN_MAESTRO 6b.3).
//!
//! ```text
//!    the COMPILER (titan-front)      writes it, once, after ITS judge said
//!                                    yes: every door the program goes
//!                                    through, and the line that opens it
//!    the KERNEL (the load gate, J2)  reads it with THIS code, and compares:
//!                                    what it says it uses / what Titan.toml
//!                                    asked / what the process is granted
//!                                    -- one subtraction each
//! ```
//!
//! ** WHAT THE KERNEL NEVER DOES WITH IT: believe it to GRANT. A permission is
//! given by the capability, always, as today; a `.bex` of C without a
//! certificate runs exactly the same. The certificate serves to NAME: when a
//! door says NO, it says which module and which LINE opened it. If a
//! certificate lies, the only thing that breaks is the explanation -- never
//! the safety. That is why `judge` can only say NO or "nothing to add".
//!
//! It is proof-carrying code (Necula, 1996) at the size of BMO-X: compiling is
//! expensive and happens once; checking is cheap and happens at every load.
//!
//! ** Where it travels: inside the `.bex`, signed with the code (the annex of
//! the MANIFEST, as a `[certificado]` section). Its own BEF2 annex is a change
//! to the format, which is Ring 0's and the owner's; until then it rides in
//! the manifest, which is already signed and already read past by the loader.
//!
//! ```toml
//!    [certificado]
//!    # lo que este programa usa de BMO-X, y desde que linea
//!    console = [6, 9]
//!    screen = [14]
//! ```
//!
//! [layer] PURE: no allocator, no `unsafe`, fixed arrays -- the kernel can
//! link it as it links the other judges of `platform/shared`.

use crate::graph::{Permission, Permissions};

/// A door of BMO-X, as a program goes through it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Door {
    /// The console of the program's OWN task: needs no permission.
    Console,
    Screen,
    Input,
    Sound,
    Gpu,
    Disk,
    Net,
}

impl Door {
    pub const ALL: [Door; 7] = [Door::Console, Door::Screen, Door::Input, Door::Sound, Door::Gpu, Door::Disk, Door::Net];

    /// The key in the certificate: the same word as in `[permissions]`.
    pub const fn key(self) -> &'static str {
        match self {
            Door::Console => "console",
            Door::Screen => "screen",
            Door::Input => "input",
            Door::Sound => "sound",
            Door::Gpu => "gpu",
            Door::Disk => "disk",
            Door::Net => "net",
        }
    }

    /// The permission this door asks for. `None`: it is the program's own.
    pub const fn needs(self) -> Option<Permission> {
        match self {
            Door::Console => None,
            Door::Screen => Some(Permission::Screen),
            Door::Input => Some(Permission::Input),
            Door::Sound => Some(Permission::Sound),
            Door::Gpu => Some(Permission::Gpu),
            Door::Disk => Some(Permission::Disk),
            Door::Net => Some(Permission::Net),
        }
    }

    fn from_key(k: &[u8]) -> Option<Door> {
        Door::ALL.into_iter().find(|d| d.key().as_bytes() == k)
    }
}

/// One use: a door, and the line of the source that opens it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Use {
    pub door: Door,
    pub line: u16,
}

/// Lines kept per door: the FIRST ones in the source, which is where to look.
/// A program with five hundred `print` still has a certificate of the same
/// size -- the kernel reads it at every load, and it must stay small.
pub const PER_DOOR: usize = 16;
pub const MAX_USES: usize = PER_DOOR * Door::ALL.len();

/// What a certificate can fail at, read back.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CertError {
    /// No `[certificado]` section: a `.bex` that is not TITAN++, or an old one.
    Missing,
    /// Line `n` of the text is not `door = [lines]`.
    BadLine(usize),
    /// The text does not fit in the buffer given to `write`.
    DoesNotFit,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Certificate {
    uses: [Use; MAX_USES],
    n: u8,
    /// Uses past `PER_DOOR`, per door (in `Door::ALL` order): counted, not kept.
    more: [u16; 7],
}

impl Default for Certificate {
    fn default() -> Self {
        Certificate::new()
    }
}

/// What the kernel's judge says. Never "granted": that is the capability's.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Verdict {
    /// The certificate, the manifest and the grant agree: nothing to add.
    Agrees,
    /// The program says it goes through a door its `Titan.toml` did not ask
    /// for -- the compiler would have refused it (U2): this `.bex` was not
    /// made by an honest compiler, or was changed after.
    Unasked(Use),
    /// Asked for, and not granted: it will hear NO at that door, and this is
    /// the line that will knock.
    Ungranted(Use),
}

impl Certificate {
    pub const fn new() -> Certificate {
        Certificate { uses: [Use { door: Door::Console, line: 0 }; MAX_USES], n: 0, more: [0; 7] }
    }

    pub fn uses(&self) -> &[Use] {
        &self.uses[..self.n as usize]
    }

    /// Records a use; the same door at the same line counts once. Past
    /// `PER_DOOR` lines of one door it is COUNTED, not kept: add the uses in
    /// source order and the kept ones are the first.
    pub fn add(&mut self, door: Door, line: u16) {
        let u = Use { door, line };
        if self.uses().contains(&u) {
            return;
        }
        if self.uses().iter().filter(|x| x.door == door).count() >= PER_DOOR {
            let k = Door::ALL.iter().position(|d| *d == door).unwrap_or(0);
            self.more[k] = self.more[k].saturating_add(1);
            return;
        }
        self.uses[self.n as usize] = u;
        self.n += 1;
    }

    /// How many uses of `door` were counted and not kept.
    pub fn more(&self, door: Door) -> u16 {
        Door::ALL.iter().position(|d| *d == door).map_or(0, |k| self.more[k])
    }

    /// The first line that opens `door`: what the kernel's NO points at (J4).
    pub fn line_of(&self, door: Door) -> Option<u16> {
        self.uses().iter().filter(|u| u.door == door).map(|u| u.line).min()
    }

    /// The `[certificado]` section, into `out`. Doors in a fixed order and
    /// lines sorted: the same program, the same bytes.
    pub fn write(&self, out: &mut [u8]) -> Result<usize, CertError> {
        let mut o = Out { buf: out, n: 0 };
        o.put(b"[certificado]\n# lo que este programa usa de BMO-X, y desde que linea (TITAN_MAESTRO 6b.3)\n")?;
        for door in Door::ALL {
            let mut lines = [0u16; MAX_USES];
            let mut k = 0;
            for u in self.uses().iter().filter(|u| u.door == door) {
                lines[k] = u.line;
                k += 1;
            }
            if k == 0 {
                continue;
            }
            lines[..k].sort_unstable();
            o.put(door.key().as_bytes())?;
            o.put(b" = [")?;
            for (i, l) in lines[..k].iter().enumerate() {
                if i > 0 {
                    o.put(b", ")?;
                }
                o.num(*l)?;
            }
            o.put(b"]\n")?;
            if self.more(door) > 0 {
                o.put(b"# y ")?;
                o.num(self.more(door))?;
                o.put(b" lineas mas por esta puerta\n")?;
            }
        }
        Ok(o.n)
    }

    /// Reads the `[certificado]` section out of a whole manifest. What it does
    /// not understand inside its section is refused with the line number,
    /// never guessed: a certificate that cannot be read names nothing.
    pub fn read(text: &[u8]) -> Result<Certificate, CertError> {
        let mut c = Certificate::new();
        let mut inside = false;
        let mut found = false;
        for (i, raw) in text.split(|&b| b == b'\n').enumerate() {
            let line = trim(raw);
            if line.is_empty() || line.starts_with(b"#") {
                continue;
            }
            if line.starts_with(b"[") {
                inside = line == b"[certificado]";
                found |= inside;
                continue;
            }
            if !inside {
                continue;
            }
            let bad = CertError::BadLine(i + 1);
            let eq = line.iter().position(|&b| b == b'=').ok_or(bad)?;
            let door = Door::from_key(trim(&line[..eq])).ok_or(bad)?;
            let list = trim(&line[eq + 1..]).strip_prefix(b"[").and_then(|l| l.strip_suffix(b"]")).ok_or(bad)?;
            for piece in list.split(|&b| b == b',').map(trim).filter(|p| !p.is_empty()) {
                c.add(door, number(piece).ok_or(bad)?);
            }
        }
        if found {
            Ok(c)
        } else {
            Err(CertError::Missing)
        }
    }
}

/// **THE KERNEL'S HALF** (J2): the certificate against what `Titan.toml`
/// asked and what the process is granted. The first disagreement, in source
/// order, so the message points at the first line to look at.
pub fn judge(c: &Certificate, asked: Permissions, granted: Permissions) -> Verdict {
    let mut first: Option<Verdict> = None;
    let mut at = u16::MAX;
    for u in c.uses() {
        let Some(p) = u.door.needs() else { continue };
        let v = if !asked.allows(p) {
            Verdict::Unasked(*u)
        } else if !granted.allows(p) {
            Verdict::Ungranted(*u)
        } else {
            continue;
        };
        if u.line < at {
            at = u.line;
            first = Some(v);
        }
    }
    first.unwrap_or(Verdict::Agrees)
}

fn trim(s: &[u8]) -> &[u8] {
    let a = s.iter().position(|c| !c.is_ascii_whitespace()).unwrap_or(s.len());
    let b = s.iter().rposition(|c| !c.is_ascii_whitespace()).map_or(a, |i| i + 1);
    &s[a..b.max(a)]
}

fn number(s: &[u8]) -> Option<u16> {
    if s.is_empty() || s.len() > 5 || !s.iter().all(u8::is_ascii_digit) {
        return None;
    }
    let v = s.iter().fold(0u32, |a, &c| a * 10 + (c - b'0') as u32);
    u16::try_from(v).ok()
}

struct Out<'a> {
    buf: &'a mut [u8],
    n: usize,
}

impl Out<'_> {
    fn put(&mut self, s: &[u8]) -> Result<(), CertError> {
        let end = self.n + s.len();
        self.buf.get_mut(self.n..end).ok_or(CertError::DoesNotFit)?.copy_from_slice(s);
        self.n = end;
        Ok(())
    }
    fn num(&mut self, v: u16) -> Result<(), CertError> {
        let mut d = [0u8; 5];
        let (mut k, mut v) = (0, v);
        loop {
            d[k] = b'0' + (v % 10) as u8;
            k += 1;
            v /= 10;
            if v == 0 {
                break;
            }
        }
        d[..k].reverse();
        self.put(&d[..k])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cert(uses: &[(Door, u16)]) -> Certificate {
        let mut c = Certificate::new();
        for &(d, l) in uses {
            c.add(d, l);
        }
        c
    }

    #[test]
    fn written_and_read_back_it_is_the_same_and_always_the_same_bytes() {
        let c = cert(&[(Door::Screen, 14), (Door::Console, 9), (Door::Console, 6), (Door::Console, 6)]);
        let mut a = [0u8; 256];
        let n = c.write(&mut a).unwrap();
        let text = core::str::from_utf8(&a[..n]).unwrap();
        assert!(text.ends_with("console = [6, 9]\nscreen = [14]\n"), "{}", text);
        // Inside a whole manifest, between other sections.
        let mut m = [0u8; 512];
        let head = b"[modulo]\nlenguaje = \"titan\"\n\n";
        m[..head.len()].copy_from_slice(head);
        m[head.len()..head.len() + n].copy_from_slice(&a[..n]);
        let back = Certificate::read(&m[..head.len() + n]).unwrap();
        assert_eq!(back.uses().len(), 3);
        assert_eq!(back.line_of(Door::Console), Some(6));
        let mut b = [0u8; 256];
        let k = back.write(&mut b).unwrap();
        assert_eq!(&b[..k], &a[..n]);
    }

    #[test]
    fn the_console_needs_nothing_and_agrees_with_nothing_granted() {
        let c = cert(&[(Door::Console, 3)]);
        assert_eq!(judge(&c, Permissions::NONE, Permissions::NONE), Verdict::Agrees);
    }

    #[test]
    fn a_door_not_asked_is_a_lie_and_one_not_granted_names_its_line() {
        let c = cert(&[(Door::Console, 2), (Door::Net, 30), (Door::Screen, 12)]);
        let screen = Permissions::NONE.with(Permission::Screen);
        // net was never asked: whoever wrote this was not the honest compiler.
        assert_eq!(judge(&c, screen, screen), Verdict::Unasked(Use { door: Door::Net, line: 30 }));
        // all asked, screen not granted: line 12 is where the NO will land.
        let asked = screen.with(Permission::Net);
        assert_eq!(judge(&c, asked, Permissions::NONE.with(Permission::Net)), Verdict::Ungranted(Use { door: Door::Screen, line: 12 }));
        assert_eq!(judge(&c, asked, asked), Verdict::Agrees);
    }

    #[test]
    fn five_hundred_prints_keep_the_first_sixteen_and_count_the_rest() {
        let mut c = Certificate::new();
        for line in 1..=500 {
            c.add(Door::Console, line);
        }
        assert_eq!((c.uses().len(), c.more(Door::Console), c.line_of(Door::Console)), (PER_DOOR, 484, Some(1)));
        let mut a = [0u8; 512];
        let n = c.write(&mut a).unwrap();
        assert!(core::str::from_utf8(&a[..n]).unwrap().contains("# y 484 lineas mas"));
    }

    #[test]
    fn what_cannot_be_read_is_refused_with_its_line() {
        assert_eq!(Certificate::read(b"[modulo]\nx = 1\n"), Err(CertError::Missing));
        assert_eq!(Certificate::read(b"[certificado]\nconsole = [3]\nwormhole = [4]\n"), Err(CertError::BadLine(3)));
        assert_eq!(Certificate::read(b"[certificado]\nconsole = [70000]\n"), Err(CertError::BadLine(2)));
        assert_eq!(Certificate::read(b"[certificado]\nconsole = 3\n"), Err(CertError::BadLine(2)));
        let mut small = [0u8; 8];
        assert_eq!(cert(&[(Door::Console, 1)]).write(&mut small), Err(CertError::DoesNotFit));
    }
}
