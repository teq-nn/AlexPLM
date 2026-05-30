// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // When git runs us as its GIT_ASKPASS helper (Issue #22), answer the credential prompt from
    // the OS keystore and exit — never launch the GUI. Detected by the env marker gitrunner sets
    // on every git spawn, which git propagates to this child.
    if app_lib::askpass::is_askpass_invocation() {
        let prompt = std::env::args().nth(1);
        std::process::exit(app_lib::askpass::run(prompt.as_deref()));
    }

    app_lib::run()
}
