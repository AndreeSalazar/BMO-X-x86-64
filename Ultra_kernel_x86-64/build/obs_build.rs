// OBS-BUILD (17-10, GCB2a de PLAN_GSP_CONTROL_BMO): el commit con que se
// compila un binario, en UNA palabra de 64 bits. Lo incluyen (`include!`) el
// build.rs del kernel y el del DIRECTOR, para que los dos la armen igual.
//
//    63      valido: git contesto
//    62      SUCIO: habia cambios sin commit en ficheros seguidos
//    0..55   los 14 primeros digitos hex del commit
//
// Sin git, o fuera de un repositorio, la palabra es 0: "desconocido", nunca un
// commit inventado. Y el guion corre en CADA build (ver `emitir`): una marca
// de limpio que se quedara de un build anterior seria un numero que miente.

fn obs_build_git(args: &[&str]) -> Option<String> {
    let salida = std::process::Command::new("git").args(args).output().ok()?;
    if !salida.status.success() {
        return None;
    }
    String::from_utf8(salida.stdout).ok()
}

fn obs_build_palabra() -> u64 {
    let Some(commit) = obs_build_git(&["rev-parse", "HEAD"]) else { return 0 };
    let Ok(digitos) = u64::from_str_radix(commit.trim().get(..14).unwrap_or(""), 16) else {
        return 0;
    };
    let Some(estado) = obs_build_git(&["status", "--porcelain", "--untracked-files=no"]) else {
        return 0;
    };
    let sucio = if estado.trim().is_empty() { 0 } else { 1 << 62 };
    1 << 63 | sucio | digitos
}

/// Deja la palabra en `BMO_OBS_BUILD` para `env!`.
fn emitir() {
    // Una ruta que no existe: cargo vuelve a correr este guion en cada build,
    // y la marca de SUCIO se calcula sobre el arbol de ESTE build. El
    // compilado solo se rehace si la palabra cambia.
    println!("cargo:rerun-if-changed=obs-build-cada-vez");
    println!("cargo:rustc-env=BMO_OBS_BUILD={}", obs_build_palabra());
}
