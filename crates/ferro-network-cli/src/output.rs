//! Output helpers shared across subcommands.
//!
//! A subcommand builds its result and calls [`emit`], which prints either
//! pretty JSON (`--json`) or the human-readable text from a closure.

use std::io::{self, Write};

use comfy_table::presets::UTF8_FULL;
use comfy_table::{ContentArrangement, Table};

/// Write `value` as pretty JSON when `json` is set, otherwise the string
/// `render_human` returns (which should end with its own newline).
///
/// # Errors
/// I/O failures writing to `out`, or JSON serialisation failures.
pub fn emit<T, F, W>(out: &mut W, value: &T, json: bool, render_human: F) -> anyhow::Result<()>
where
    T: serde::Serialize,
    F: FnOnce() -> String,
    W: Write,
{
    if json {
        serde_json::to_writer_pretty(&mut *out, value)?;
        out.write_all(b"\n")?;
    } else {
        out.write_all(render_human().as_bytes())?;
    }
    Ok(())
}

/// [`emit`] to a locked stdout.
///
/// # Errors
/// As [`emit`].
pub fn emit_stdout<T, F>(value: &T, json: bool, render_human: F) -> anyhow::Result<()>
where
    T: serde::Serialize,
    F: FnOnce() -> String,
{
    emit(&mut io::stdout().lock(), value, json, render_human)
}

/// Render rows as a human-readable table with a trailing newline.
///
/// Rows longer than `headers` are truncated and shorter ones padded (with
/// a `warn!`), so a renderer bug never panics. Callers short-circuit
/// empty lists with their own "(no ...)" text.
#[must_use]
pub fn table(headers: &[&str], rows: &[Vec<String>]) -> String {
    let cols = headers.len();
    let mut table = Table::new();
    // UTF8_FULL: visible row/column separation without relying on
    // terminal styling. Dynamic arrangement wraps long cells to the
    // terminal width.
    table
        .load_style(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(headers.iter().copied());

    for (index, row) in rows.iter().enumerate() {
        if row.len() != cols {
            log::warn!(
                "table row {index} has {} cells for {cols} headers; truncating/padding",
                row.len()
            );
        }
        let mut cells: Vec<&str> = row.iter().map(String::as_str).take(cols).collect();
        cells.resize(cols, "");
        table.add_row(cells);
    }

    format!("{table}\n")
}

#[cfg(test)]
mod tests {
    use super::{emit, table};

    #[test]
    fn emit_switches_between_json_and_human() {
        let value = serde_json::json!({ "applicationVersion": "10.0.160" });

        let mut out = Vec::new();
        emit(&mut out, &value, true, || unreachable!()).expect("emits");
        assert_eq!(
            String::from_utf8(out).expect("utf8"),
            "{\n  \"applicationVersion\": \"10.0.160\"\n}\n"
        );

        let mut out = Vec::new();
        emit(&mut out, &value, false, || "human\n".to_string()).expect("emits");
        assert_eq!(String::from_utf8(out).expect("utf8"), "human\n");
    }

    #[test]
    fn table_renders_headers_and_cells() {
        let out = table(
            &["ID", "NAME"],
            &[
                vec!["net-1".into(), "Default".into()],
                vec!["net-2".into(), "Lab".into()],
            ],
        );
        for needle in ["ID", "NAME", "net-1", "Default", "Lab"] {
            assert!(out.contains(needle), "{needle} missing: {out}");
        }
        // A preset change should be a deliberate test update.
        assert!(out.contains('─'), "expected a UTF8_FULL border: {out}");
        assert!(out.ends_with('\n'));
    }

    #[test]
    fn table_truncates_and_pads_rows() {
        let out = table(
            &["ID", "NAME"],
            &[
                vec!["a".into(), "b".into(), "EXTRA".into()],
                vec!["c".into()],
            ],
        );
        assert!(!out.contains("EXTRA"), "extra cell rendered: {out}");
        assert!(out.contains('c'));
    }
}
