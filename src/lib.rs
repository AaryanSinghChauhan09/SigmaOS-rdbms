//! SigmaOS RDBMS component — database interoperability primitives.
//!
//! SQL identifier quoting per dialect, connection-string parsing,
//! and schema-qualified name handling. Safe Rust, no unsafe code.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::fmt;

/// SQL dialects supported for identifier quoting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialect {
    /// PostgreSQL — double quotes.
    Postgres,
    /// MySQL/MariaDB — backticks.
    MySql,
    /// SQL Server / Access — square brackets.
    SqlServer,
    /// SQLite/Oracle/DB2 — double quotes.
    Standard,
}

/// Errors produced by the RDBMS component.
#[derive(Debug)]
pub enum RdbmsError {
    /// Identifier contains a NUL or is empty.
    InvalidIdentifier(String),
    /// Connection string could not be parsed.
    BadConnString(String),
}

impl fmt::Display for RdbmsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidIdentifier(i) => write!(f, "invalid identifier: {i:?}"),
            Self::BadConnString(s) => write!(f, "bad connection string: {s}"),
        }
    }
}

impl std::error::Error for RdbmsError {}

/// Quote a single identifier for the given dialect.
pub fn quote_ident(dialect: Dialect, ident: &str) -> Result<String, RdbmsError> {
    if ident.is_empty() || ident.contains('\0') {
        return Err(RdbmsError::InvalidIdentifier(ident.to_string()));
    }
    Ok(match dialect {
        Dialect::Postgres | Dialect::Standard => format!("\"{}\"", ident.replace('"', "\"\"")),
        Dialect::MySql => format!("`{}`", ident.replace('`', "``")),
        Dialect::SqlServer => format!("[{}]", ident.replace(']', "]]")),
    })
}

/// Quote a possibly schema-qualified name: `schema.table` → quoted parts.
pub fn quote_qualified(dialect: Dialect, name: &str) -> Result<String, RdbmsError> {
    let parts: Vec<&str> = name.split('.').collect();
    if parts.iter().any(|p| p.is_empty()) {
        return Err(RdbmsError::InvalidIdentifier(name.to_string()));
    }
    parts.iter()
        .map(|p| quote_ident(dialect, p))
        .collect::<Result<Vec<_>, _>>()
        .map(|v| v.join("."))
}

/// Parsed key/value connection parameters.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ConnParams {
    /// host=...
    pub host: Option<String>,
    /// port=...
    pub port: Option<u16>,
    /// dbname=... or database=...
    pub database: Option<String>,
    /// user=...
    pub user: Option<String>,
    /// password=... (caller must not log this)
    pub password: Option<String>,
    /// Extra parameters preserved verbatim.
    pub extra: BTreeMap<String, String>,
}

/// Parse a libpq-style `key=value` connection string.
/// Values may be single-quoted with `''` escape; whitespace separates pairs.
pub fn parse_conn_string(s: &str) -> Result<ConnParams, RdbmsError> {
    let mut p = ConnParams::default();
    let bytes = s.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() { i += 1; }
        if i >= bytes.len() { break; }
        let start = i;
        while i < bytes.len() && bytes[i] != b'=' && !bytes[i].is_ascii_whitespace() { i += 1; }
        if i >= bytes.len() || bytes[i] != b'=' {
            return Err(RdbmsError::BadConnString(s.to_string()));
        }
        let key = &s[start..i];
        i += 1;
        let value = if i < bytes.len() && bytes[i] == b'\'' {
            i += 1;
            let mut v = String::new();
            while i < bytes.len() {
                if bytes[i] == b'\'' {
                    if i + 1 < bytes.len() && bytes[i + 1] == b'\'' {
                        v.push('\''); i += 2; continue;
                    }
                    i += 1; break;
                }
                v.push(bytes[i] as char); i += 1;
            }
            v
        } else {
            let vs = i;
            while i < bytes.len() && !bytes[i].is_ascii_whitespace() { i += 1; }
            s[vs..i].to_string()
        };
        match key {
            "host" => p.host = Some(value),
            "port" => p.port = value.parse().ok(),
            "dbname" | "database" => p.database = Some(value),
            "user" => p.user = Some(value),
            "password" => p.password = Some(value),
            _ => { p.extra.insert(key.to_string(), value); }
        }
    }
    Ok(p)
}

/// Build a safe `key=value` connection string (password kept verbatim —
/// caller is responsible for not logging the result).
pub fn build_conn_string(p: &ConnParams) -> String {
    let mut parts = Vec::new();
    if let Some(h) = &p.host { parts.push(format!("host={h}")); }
    if let Some(port) = p.port { parts.push(format!("port={port}")); }
    if let Some(d) = &p.database { parts.push(format!("dbname={d}")); }
    if let Some(u) = &p.user { parts.push(format!("user={u}")); }
    if let Some(pw) = &p.password { parts.push(format!("password={}", quote_value(pw))); }
    for (k, v) in &p.extra { parts.push(format!("{k}={}", quote_value(v))); }
    parts.join(" ")
}

fn quote_value(v: &str) -> String {
    if v.chars().any(|c| c.is_ascii_whitespace() || c == '\'') {
        format!("'{}'", v.replace('\'', "''"))
    } else {
        v.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_per_dialect() {
        assert_eq!(quote_ident(Dialect::Postgres, "my table").unwrap(), "\"my table\"");
        assert_eq!(quote_ident(Dialect::MySql, "weird`name").unwrap(), "`weird``name`");
        assert_eq!(quote_ident(Dialect::SqlServer, "a]b").unwrap(), "[a]]b]");
        assert_eq!(quote_qualified(Dialect::Postgres, "public.users").unwrap(), "\"public\".\"users\"");
    }

    #[test]
    fn parses_conn_string() {
        let p = parse_conn_string("host=db.local port=5432 dbname=app user=alice password='p@ss word' sslmode=require").unwrap();
        assert_eq!(p.host.as_deref(), Some("db.local"));
        assert_eq!(p.port, Some(5432));
        assert_eq!(p.database.as_deref(), Some("app"));
        assert_eq!(p.user.as_deref(), Some("alice"));
        assert_eq!(p.password.as_deref(), Some("p@ss word"));
        assert_eq!(p.extra.get("sslmode").map(String::as_str), Some("require"));
    }

    #[test]
    fn rejects_empty_ident() {
        assert!(quote_ident(Dialect::Postgres, "").is_err());
        assert!(quote_qualified(Dialect::Postgres, "a..b").is_err());
    }
}
