use std::io::Write;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format(|buf, record| {
            writeln!(
                buf,
                "{} [{}] - {}",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                record.level(),
                record.args()
            )
        })
        .init();

    if let Err(e) = tt::cli::run() {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
