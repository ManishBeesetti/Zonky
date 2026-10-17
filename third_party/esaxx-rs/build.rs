#[cfg(feature = "cpp")]
#[cfg(not(target_os = "macos"))]
fn main() {
    let mut build = cc::Build::new();
    build.cpp(true);
    if !cfg!(target_env = "msvc") {
        build.flag("-std=c++11");
    }
    // Use the toolchain default C runtime linkage.
    // For MSVC this avoids hard-forcing /MT, which can conflict with crates
    // built with /MD (for example llama-cpp-sys in this workspace).
    build
        .static_crt(false)
        .file("src/esaxx.cpp")
        .include("src")
        .compile("esaxx");
}

#[cfg(feature = "cpp")]
#[cfg(target_os = "macos")]
fn main() {
    cc::Build::new()
        .cpp(true)
        .flag("-std=c++11")
        .flag("-stdlib=libc++")
        .static_crt(false)
        .file("src/esaxx.cpp")
        .include("src")
        .compile("esaxx");
}

#[cfg(not(feature = "cpp"))]
fn main() {}
