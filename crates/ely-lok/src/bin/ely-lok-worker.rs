use std::io;

fn main() {
    // As the app: ELY_LOG, info by default.
    env_logger::Builder::from_env(env_logger::Env::new().filter_or("ELY_LOG", "info")).init();
    let args: Vec<String> = std::env::args().collect();
    let Some((install, profile)) = args.get(1).zip(args.get(2)) else {
        log::error!("usage: ely-lok-worker <install-path> <profile-dir>");
        std::process::exit(2);
    };
    let input = io::stdin();
    let output = io::stdout();
    #[cfg(windows)]
    if !args.iter().any(|arg| arg == "--bootstrap-gate") {
        log::error!("lok-worker: supervised bootstrap is required");
        std::process::exit(2);
    }
    #[cfg(windows)]
    {
        match ely_lok::worker_bootstrap() {
            Ok(()) => {}
            Err(error) => {
                log::error!("lok-worker: bootstrap gate failed: {error}");
                std::process::exit(1);
            }
        }
    }
    if let Err(error) = ely_lok::worker_run(install, profile, input, output) {
        // A record, so the app's relay passes it on.
        log::error!("lok-worker: {error}");
        std::process::exit(1);
    }
}
