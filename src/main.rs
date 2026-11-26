//! sigmaos-rdbms CLI — quote identifiers / parse conn strings.

use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("quote") => {
            let dialect = match args.get(2).map(String::as_str) {
                Some("mysql") => sigmaos_rdbms::Dialect::MySql,
                Some("sqlserver") => sigmaos_rdbms::Dialect::SqlServer,
                _ => sigmaos_rdbms::Dialect::Postgres,
            };
            let name = args
                .get(3)
                .ok_or("usage: sigmaos-rdbms quote <dialect> <name>")?;
            println!("{}", sigmaos_rdbms::quote_qualified(dialect, name)?);
        }
        Some("parse") => {
            let cs = args
                .get(2)
                .ok_or("usage: sigmaos-rdbms parse <connstring>")?;
            let p = sigmaos_rdbms::parse_conn_string(cs)?;
            println!(
                "host={:?} port={:?} db={:?} user={:?}",
                p.host, p.port, p.database, p.user
            );
        }
        _ => eprintln!("usage: sigmaos-rdbms [quote <dialect> <name>|parse <connstring>]"),
    }
    Ok(())
}
