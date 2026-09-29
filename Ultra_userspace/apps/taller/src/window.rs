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
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Input {
    /// A mouse button changed: where (in our pixels), which buttons, down or up.
    Mouse { x: i32, y: i32, buttons: u8, down: bool },
    /// A letter, already cooked by the kernel's keyboard map (Latin-1).
    Char(u8),
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
        // The block lives until the process dies: the DIRECTOR composes from it.
        core::mem::forget(block);
        let parent = bmo::mi_padre();
        if parent == 0 || !bmo::offer(handle, 0, bytes, parent) {
            return None;
        }
        Some(Window { base, mailbox, px: (base + bmo::SUP_CABECERA) as *mut u32, w, h })
    }

    /// The frame is finished: raise the sequence so the DIRECTOR copies it.
    pub fn present(&self) {
        let at = self.base + 4 * bmo::SUP_CAMPO_SECUENCIA;
        put(at, get(at).wrapping_add(1));
    }

    /// The next click or letter, if any. The DIRECTOR writes the head; we own
    /// the tail.
    pub fn next(&self) -> Option<Input> {
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
