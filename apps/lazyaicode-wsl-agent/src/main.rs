//! O backend git que roda dentro de uma distro WSL para o app do Windows.
//! `lazyaicode-agent serve` fala `lazyaicode_git::proto` no stdio.

fn main() {
    // helper askpass do ssh-keygen ao assinar commits dentro da distro
    if lazyaicode_git::answer_askpass() {
        return;
    }
    match std::env::args().nth(1).as_deref() {
        Some("--version") => println!("{}", lazyaicode_git::wsl::agent_version()),
        Some("serve") => lazyaicode_git::agent_server::serve_stdio(),
        _ => {
            eprintln!("uso: lazyaicode-agent serve | --version");
            std::process::exit(2);
        }
    }
}
