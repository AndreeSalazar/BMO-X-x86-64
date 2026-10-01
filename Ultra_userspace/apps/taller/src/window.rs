//! **THE WINDOW** -- a BSUP surface lent to the DIRECTOR, its mailbox and the
//! pointer. The shape is `bmo_abi::syscalls::surface::superficie`, through the
//! `SUP_*` copies of `bmo-userland` (R4 of `contrato` compares them).
//!
//! ```text
//!    header   MAGIC  width  height  stride  BGRA32  sequence  mailbox  slots
//!    pixels   width * height * 4, BGRA
//!    mailbox  head  tail  | +8 pointer x,y | +12 buttons, inside, VIEW
//!             then `slots` events of 8 bytes, written by the DIRECTOR
//! ```
//!
//! Clicks and letters arrive as EVENTS; the pointer is a STATE the DIRECTOR
//! overwrites every frame (`Surface::puntero`), so dragging reads it directly
//! instead of waiting for motion events that do not exist.

use bmo_userland as bmo;

/// Mailbox slots: a power of two, as the DIRECTOR demands.
const SLOTS: u64 = 64;

/// Bit 8 of a raw event: there is one. Bit 9: the key or button went DOWN.
const DOWN: u64 = 0x200;

pub struct Window {
    base: u64,
    mailbox: u64,
    pub px: *mut u32,
    pub w: u32,
    pub h: u32,
    /// The blocks offered before a CONFIGURE: the DIRECTOR may still be
    /// composing from them until it marks the new one TAKEN. Released then,
    /// never before (`bmo-golpe/src/configure.rs`, step 4).
    old: [Option<bmo::Memoria>; 4],
    /// The block in use, kept to hand it to `old` on the next resize.
    /// (TALLER never resizes: for it, this only keeps the block alive.)
    #[allow(dead_code)]
    block: Option<bmo::Memoria>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Input {
    /// A mouse button changed: where (in our pixels), which buttons, down or up.
    Mouse { x: i32, y: i32, buttons: u8, down: bool },
    /// A letter, already cooked by the kernel's keyboard map (Latin-1).
    Char(u8),
    /// CONFIGURE: the DIRECTOR says the room we have now (maximized, full
    /// screen, back to a window). `resize` answers it; ignoring it is safe.
    Resize { w: u32, h: u32 },
}

#[derive(Clone, Copy)]
pub struct Pointer {
    pub x: i32,
    pub y: i32,
    pub buttons: u8,
    pub inside: bool,
    /// `SUP_VISTA_*`: if it is not `SE_VE`, nobody would see a new frame.
    pub view: u8,
}

fn put(addr: u64, v: u32) {
    // SAFETY: every caller passes an address inside our own block, which
    // lives until the process dies (the DIRECTOR is reading it).
    unsafe { core::ptr::write_volatile(addr as *mut u32, v) };
}

fn get(addr: u64) -> u32 {
    // SAFETY: as in `put`.
    unsafe { core::ptr::read_volatile(addr as *const u32) }
}

impl Window {
    /// Asks for the block, writes the header and offers it to whoever
    /// launched us (the DIRECTOR). `None` says which step failed through the
    /// console, and the caller exits.
    pub fn open(w: u32, h: u32) -> Option<Window> {
        let (block, base, mailbox) = Self::offered(w, h)?;
        Some(Window { base, mailbox, px: (base + bmo::SUP_CABECERA) as *mut u32, w, h, old: [None, None, None, None], block: Some(block) })
    }

    /// **Answers a CONFIGURE**: a new surface of `w x h`, offered while the
    /// old one stays alive. From here on we paint into the new one. `false`:
    /// same size, or no memory -- we stay as we were, which the DIRECTOR
    /// also accepts (it centers us).
    #[allow(dead_code)] // TALLER keeps its size; LUDOTECA answers.
    pub fn resize(&mut self, w: u32, h: u32) -> bool {
        if (w, h) == (self.w, self.h) || w == 0 || h == 0 {
            return false;
        }
        let Some((block, base, mailbox)) = Self::offered(w, h) else { return false };
        // Four resizes before the DIRECTOR takes one is not a real case; if it
        // happens, the oldest is forgotten (lent forever) rather than freed.
        if let Some(b) = self.block.replace(block) {
            if let Some(hueco) = self.old.iter_mut().find(|o| o.is_none()) {
                *hueco = Some(b);
            } else {
                core::mem::forget(b);
            }
        }
        self.base = base;
        self.mailbox = mailbox;
        self.px = (base + bmo::SUP_CABECERA) as *mut u32;
        self.w = w;
        self.h = h;
        true
    }

    /// The old blocks go back once the DIRECTOR took the new one. A block it
    /// still holds stays ours until the next look: never released lent.
    fn release_old(&mut self) {
        if self.old.iter().all(Option::is_none) || get(self.mailbox + 12) as u64 & bmo::SUP_TOMADA == 0 {
            return;
        }
        for o in self.old.iter_mut() {
            if let Some(b) = o.take() {
                if let Err(b) = b.soltar_esperando(1_000_000) {
                    *o = Some(b);
                }
            }
        }
    }

    /// A block of `w x h` with its header and an empty mailbox, offered to
    /// whoever launched us.
    fn offered(w: u32, h: u32) -> Option<(bmo::Memoria, u64, u64)> {
        let pixels = w as u64 * h as u64 * 4;
        let mailbox_at = bmo::SUP_CABECERA + pixels;
        let bytes = mailbox_at + bmo::SUP_BUZON_CABECERA + SLOTS * bmo::SUP_BUZON_RANURA;
        let block = bmo::Memoria::request(bytes)?;
        let base = block.base() as u64;
        let handle = block.handle();
        for (i, v) in [bmo::SUP_MAGIC, w as u64, h as u64, w as u64, bmo::SUP_BGRA32, 0, mailbox_at, SLOTS]
            .into_iter()
            .enumerate()
        {
            put(base + 4 * i as u64, v as u32);
        }
        let mailbox = base + mailbox_at;
        // Head, tail and state at zero BEFORE offering.
        for i in 0..4 {
            put(mailbox + 4 * i, 0);
        }
        let parent = bmo::mi_padre();
        if parent == 0 || !bmo::offer(handle, 0, bytes, parent) {
            return None;
        }
        Some((block, base, mailbox))
    }

    /// The frame is finished: raise the sequence so the DIRECTOR copies it.
    pub fn present(&self) {
        let at = self.base + 4 * bmo::SUP_CAMPO_SECUENCIA;
        put(at, get(at).wrapping_add(1));
    }

    /// The next click or letter, if any. The DIRECTOR writes the head; we own
    /// the tail.
    pub fn next(&mut self) -> Option<Input> {
        self.release_old();
        let head = get(self.mailbox);
        let tail = get(self.mailbox + 4);
        if head == tail {
            return None;
        }
        let mask = SLOTS as u32 - 1;
        let slot = self.mailbox + bmo::SUP_BUZON_CABECERA + (tail & mask) as u64 * bmo::SUP_BUZON_RANURA;
        // SAFETY: the slot is inside the mailbox of our own block.
        let e = unsafe { core::ptr::read_volatile(slot as *const u64) };
        put(self.mailbox + 4, (tail + 1) & mask);
        if e & bmo::SUP_EV_CONFIGURE != 0 {
            return Some(Input::Resize { w: ((e >> 16) & 0xFFFF) as u32, h: ((e >> 32) & 0xFFFF) as u32 });
        }
        if e & bmo::SUP_EV_RATON != 0 {
            return Some(Input::Mouse {
                x: ((e >> 16) & 0xFFFF) as i32,
                y: ((e >> 32) & 0xFFFF) as i32,
                buttons: (e & 0xFF) as u8,
                down: e & DOWN != 0,
            });
        }
        if e & bmo::SUP_EV_CARACTER != 0 {
            return Some(Input::Char((e & 0xFF) as u8));
        }
        // A window change or a raw scancode: nothing F1 uses today. Say
        // "something happened" so the caller keeps draining.
        Some(Input::Char(0))
    }

    /// Where the pointer is NOW, and whether anyone sees us.
    pub fn pointer(&self) -> Pointer {
        let xy = get(self.mailbox + 8);
        let st = get(self.mailbox + 12);
        Pointer {
            x: (xy & 0xFFFF) as i32,
            y: (xy >> 16) as i32,
            buttons: (st & 0xFF) as u8,
            inside: (st >> 8) & 1 != 0,
            view: ((st >> 16) & 0xFF) as u8,
        }
    }
}
