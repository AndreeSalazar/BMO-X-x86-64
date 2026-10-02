//! Para `ficheros::pruebas_capa` y `pruebas_mapeo_archive`.

extern crate std;

/// **Un TEB para el hilo de la prueba** (02-10). La casa lee el TEB por
/// `gs:[0x30]` (`kernel32::teb`, el LastError). En un Windows anfitrion
/// GS ya es el TEB de verdad del hilo y esto no hace nada; en Linux GS
/// esta a cero y la prueba caia con SIGSEGV en `set_last_error`. Como el
/// banco (`tests/corre.rs`): TEB y PEB nuevos y `arch_prctl(ARCH_SET_GS)`.
pub(super) fn teb_de_prueba() {
    #[cfg(target_os = "linux")]
    {
        use bmo_proton_x::teb;
        let forma = std::alloc::Layout::from_size_align(teb::TEB_BYTES + teb::PEB_BYTES, 4096).unwrap();
        // SAFETY: `forma` no mide cero; el bloque vive lo que el proceso.
        let mem = unsafe { std::alloc::alloc_zeroed(forma) };
        assert!(!mem.is_null());
        let rsp: u64;
        // SAFETY: leer el rsp de este hilo.
        unsafe { core::arch::asm!("mov {}, rsp", out(reg) rsp) };
        let h = teb::Hilo {
            teb: mem as u64,
            peb: mem as u64 + teb::TEB_BYTES as u64,
            pila_tope: rsp + (64 << 10),
            pila_fondo: rsp - (256 << 10),
            proceso: 7,
            hilo: 42,
            base_imagen: 0,
        };
        // SAFETY: TEB_BYTES + PEB_BYTES recien pedidos, R+W.
        let t = unsafe { core::slice::from_raw_parts_mut(mem, teb::TEB_BYTES + teb::PEB_BYTES) };
        let (tb, pb) = t.split_at_mut(teb::TEB_BYTES);
        teb::escribir_teb(tb, &h);
        teb::escribir_peb(pb, &h);
        let r: u64;
        // SAFETY: arch_prctl(ARCH_SET_GS, mem): el GS de ESTE hilo.
        unsafe {
            core::arch::asm!("syscall", inlateout("rax") 158u64 => r, in("rdi") 0x1001u64, in("rsi") mem as u64,
                lateout("rcx") _, lateout("r11") _, options(nostack));
        }
        assert_eq!(r, 0, "arch_prctl(ARCH_SET_GS) dijo {r:#x}");
    }
}
