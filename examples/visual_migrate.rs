//! DEVELOPMENT ONLY. Synthetic migration acceptance; not an installed CLI command.
#[cfg(unix)]
fn main() {
    let a: Vec<_> = std::env::args().collect();
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        if a.len() < 4 {
            return Err("usage: visual_migrate prepare ROOT NEW_BACKUP | apply/status ROOT BACKUP PLAN_SHA256".into());
        }
        let root = std::path::Path::new(&a[2]);
        let backup = std::path::Path::new(&a[3]);
        match a[1].as_str() {
            "prepare" if a.len() == 4 => {
                println!("{}", gil::visual_migration::prepare(root, backup)?)
            }
            "apply" if a.len() == 5 => {
                gil::visual_migration::apply(root, backup, &a[4])?;
                println!("applied; backup retained");
            }
            "status" if a.len() == 5 => {
                println!("{}", gil::visual_migration::status(root, backup, &a[4])?)
            }
            _ => return Err("invalid command/arguments".into()),
        }
        Ok(())
    })();
    if let Err(e) = result {
        eprintln!(
            "migration stopped: {e}. Preserve all files; do not retry/restore automatically."
        );
        std::process::exit(1);
    }
}
#[cfg(not(unix))]
fn main() {
    eprintln!("development migration is not verified on this platform");
    std::process::exit(1);
}
