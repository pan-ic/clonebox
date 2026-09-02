use clonebox_core::container::exec;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut a = std::env::args();
    a.next();
    let id = a.next().ok_or("missing container id")?;
    let cmd: Vec<String> = a.collect();

    exec(&id, cmd)?;
    Ok(())
}
