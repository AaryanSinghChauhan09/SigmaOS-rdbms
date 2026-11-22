//! SigmaOS component binary: rdbms

use sigmaos_rdbms::Component;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    Component::new().operate()?;
    println!("sigmaos-rdbms: ok");
    Ok(())
}
