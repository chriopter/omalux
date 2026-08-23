pub(crate) fn prepare() -> bool {
    let arguments: Vec<String> = std::env::args().collect();
    if arguments
        .iter()
        .any(|argument| argument == "--help" || argument == "-h")
    {
        print_help();
        return false;
    }
    if arguments
        .iter()
        .any(|argument| argument == "--version" || argument == "-V")
    {
        println!("grainroom-gui {}", env!("CARGO_PKG_VERSION"));
        return false;
    }
    if arguments.iter().any(|argument| argument == "--headless")
        && std::env::var_os("QT_QPA_PLATFORM").is_none()
    {
        // SAFETY: this runs before Qt or any application threads are created.
        unsafe { std::env::set_var("QT_QPA_PLATFORM", "offscreen") };
    }
    true
}

fn print_help() {
    println!(
        "Grainroom — Omarchy photo developer\n\n\
Usage:\n  grainroom-gui [OPTIONS]\n\n\
Options:\n  --input PATH         Open JPEG, PNG, BMP, or camera RAW\n  --output PATH        Export without a save dialog\n  --format FORMAT      original, jpeg/jpg, or heic/heif (default: inferred)\n  --quality 1..100     JPEG/HEIC quality (default: 90)\n  --grain 0..100       Grain amount (default: 24)\n  --grain-size ISO     Grain size, 20..6400 (default: 4000)\n  --midtones 0..100    Midtone grain response (default: 100)\n  --headless           Run export without showing a window\n  -h, --help           Show this help\n  -V, --version        Show the version\n\n\
Examples:\n  grainroom-gui --input photo.jpg\n  grainroom-gui --headless --input photo.jpg --output out.heic \\
    --format heic --quality 90 --grain 24 --grain-size 4000"
    );
}
