use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};

use turso_ext::{
    register_extension, scalar, Connection, ResultCode, StepResult, VTabCursor, VTabKind,
    VTabModule, VTabModuleDerive, VTable, Value, ValueType,
};

register_extension! {
    scalars: { tts_extension_loaded, tts_time_bucket_ns },
    vtabs: { TtsHypertableModule },
}

#[scalar(name = "tts_extension_loaded")]
fn tts_extension_loaded(&self, _args: &[Value]) -> Value {
    Value::from_text("turso-timeseries-ext-native".to_string())
}

#[scalar(name = "tts_time_bucket_ns")]
fn tts_time_bucket_ns(&self, args: &[Value]) -> Value {
    let Some(ts_ns) = args.first().and_then(Value::to_integer) else {
        return Value::null();
    };
    let Some(width_ns) = args.get(1).and_then(Value::to_integer) else {
        return Value::null();
    };
    if width_ns <= 0 {
        return Value::null();
    }

    Value::from_integer(ts_ns - ts_ns.rem_euclid(width_ns))
}

#[derive(Debug, VTabModuleDerive, Default)]
struct TtsHypertableModule;

impl VTabModule for TtsHypertableModule {
    // Keep the FFI table payload pointer-sized. With the full state returned by
    // value, pre.30 can hand back an empty schema pointer for this module.
    type Table = Box<TtsHypertableTable>;
    const NAME: &'static str = "tts_hypertable";
    const READONLY: bool = false;
    const VTAB_KIND: VTabKind = VTabKind::VirtualTable;

    fn create(args: &[Value]) -> Result<(String, Self::Table), ResultCode> {
        let table_name = parse_table_name(args)?;
        let chunk_interval_ns = parse_chunk_interval_ns(args)?;
        // Turso follows SQLite's virtual table convention: the declared schema
        // table name is ignored, so use the upstream examples' `x` placeholder.
        let schema = "CREATE TABLE x (
            ts_ns INTEGER,
            value_real REAL,
            quality INTEGER
        )"
        .to_string();

        Ok((
            schema,
            Box::new(TtsHypertableTable {
                table_name,
                chunk_interval_ns,
                conn: Mutex::new(None),
            }),
        ))
    }
}

#[derive(Debug)]
struct TtsHypertableTable {
    table_name: String,
    chunk_interval_ns: i64,
    conn: Mutex<Option<Arc<Connection>>>,
}

impl VTable for Box<TtsHypertableTable> {
    type Cursor = TtsHypertableCursor;
    type Error = String;

    fn open(&self, conn: Option<Arc<Connection>>) -> Result<Self::Cursor, Self::Error> {
        if let Some(conn) = conn.clone() {
            *self.conn.lock().map_err(|_| "connection lock poisoned")? = Some(conn);
        }

        Ok(TtsHypertableCursor {
            conn,
            table_name: self.table_name.clone(),
            chunk_interval_ns: self.chunk_interval_ns,
            rows: Vec::new(),
            pos: 0,
        })
    }

    fn insert(&mut self, args: &[Value]) -> Result<i64, Self::Error> {
        let conn = self.connection()?;
        ensure_storage(&conn, &self.table_name, self.chunk_interval_ns)?;
        let row = parse_row_args(args)?;
        let values = row.to_values(&self.table_name);
        let rowid = conn
            .execute(
                "INSERT INTO _tts_hypertable_rows (table_name, ts_ns, value_real, quality) VALUES (?, ?, ?, ?)",
                &values,
            )
            .map_err(|code| code.to_string())?
            .unwrap_or(0);
        Ok(rowid as i64)
    }

    fn update(&mut self, rowid: i64, args: &[Value]) -> Result<(), Self::Error> {
        let conn = self.connection()?;
        ensure_storage(&conn, &self.table_name, self.chunk_interval_ns)?;
        let row = parse_row_args(args)?;
        let values = vec![
            Value::from_integer(row.ts_ns),
            optional_float_value(row.value_real),
            Value::from_integer(row.quality),
            Value::from_text(self.table_name.clone()),
            Value::from_integer(rowid),
        ];
        conn.execute(
            "UPDATE _tts_hypertable_rows SET ts_ns = ?, value_real = ?, quality = ? WHERE table_name = ? AND rowid = ?",
            &values,
        )
        .map_err(|code| code.to_string())?;
        Ok(())
    }

    fn delete(&mut self, rowid: i64) -> Result<(), Self::Error> {
        let conn = self.connection()?;
        ensure_storage(&conn, &self.table_name, self.chunk_interval_ns)?;
        let values = vec![
            Value::from_text(self.table_name.clone()),
            Value::from_integer(rowid),
        ];
        conn.execute(
            "DELETE FROM _tts_hypertable_rows WHERE table_name = ? AND rowid = ?",
            &values,
        )
        .map_err(|code| code.to_string())?;
        Ok(())
    }
}

impl TtsHypertableTable {
    fn connection(&self) -> Result<Arc<Connection>, String> {
        self.conn
            .lock()
            .map_err(|_| "connection lock poisoned".to_string())?
            .clone()
            .ok_or_else(|| "tts_hypertable connection is not open".to_string())
    }
}

#[derive(Debug)]
struct TtsHypertableCursor {
    conn: Option<Arc<Connection>>,
    table_name: String,
    chunk_interval_ns: i64,
    rows: Vec<TtsHypertableRow>,
    pos: usize,
}

impl VTabCursor for TtsHypertableCursor {
    type Error = String;

    fn filter(&mut self, _args: &[Value], _idx_info: Option<(&str, i32)>) -> ResultCode {
        let Some(conn) = self.conn.clone() else {
            return ResultCode::Error;
        };
        if ensure_storage(&conn, &self.table_name, self.chunk_interval_ns).is_err() {
            return ResultCode::Error;
        }

        let mut stmt = match conn.prepare(
            "SELECT rowid, ts_ns, value_real, quality \
             FROM _tts_hypertable_rows \
             WHERE table_name = ? \
             ORDER BY ts_ns, rowid",
        ) {
            Ok(stmt) => stmt,
            Err(code) => return code,
        };
        stmt.bind_at(
            NonZeroUsize::new(1).expect("nonzero bind index"),
            Value::from_text(self.table_name.clone()),
        );

        self.rows.clear();
        self.pos = 0;
        loop {
            match stmt.step() {
                StepResult::Row => {
                    let row = stmt.get_row();
                    self.rows.push(TtsHypertableRow {
                        rowid: row.first().and_then(Value::to_integer).unwrap_or_default(),
                        ts_ns: row.get(1).and_then(Value::to_integer).unwrap_or_default(),
                        value_real: match row.get(2).map(Value::value_type) {
                            Some(ValueType::Null) | None => None,
                            _ => row.get(2).and_then(Value::to_float),
                        },
                        quality: row.get(3).and_then(Value::to_integer).unwrap_or_default(),
                    });
                }
                StepResult::Done => return ResultCode::OK,
                StepResult::Interrupt => return ResultCode::Interrupt,
                StepResult::Busy => return ResultCode::Busy,
                StepResult::Error => return ResultCode::Error,
            }
        }
    }

    fn rowid(&self) -> i64 {
        self.current().map(|row| row.rowid).unwrap_or_default()
    }

    fn column(&self, idx: u32) -> Result<Value, Self::Error> {
        let Some(row) = self.current() else {
            return Ok(Value::null());
        };
        Ok(match idx {
            0 => Value::from_integer(row.ts_ns),
            1 => optional_float_value(row.value_real),
            2 => Value::from_integer(row.quality),
            _ => Value::null(),
        })
    }

    fn eof(&self) -> bool {
        self.pos >= self.rows.len()
    }

    fn next(&mut self) -> ResultCode {
        self.pos = self.pos.saturating_add(1);
        ResultCode::OK
    }
}

impl TtsHypertableCursor {
    fn current(&self) -> Option<&TtsHypertableRow> {
        self.rows.get(self.pos)
    }
}

#[derive(Debug)]
struct TtsHypertableRow {
    rowid: i64,
    ts_ns: i64,
    value_real: Option<f64>,
    quality: i64,
}

impl TtsHypertableRow {
    fn to_values(&self, table_name: &str) -> Vec<Value> {
        vec![
            Value::from_text(table_name.to_string()),
            Value::from_integer(self.ts_ns),
            optional_float_value(self.value_real),
            Value::from_integer(self.quality),
        ]
    }
}

fn parse_table_name(args: &[Value]) -> Result<String, ResultCode> {
    let table_name = args
        .first()
        .and_then(Value::to_text_coerced)
        .map(|value| trim_sql_quotes(&value))
        .ok_or(ResultCode::InvalidArgs)?;

    if is_simple_identifier(&table_name) {
        Ok(table_name)
    } else {
        Err(ResultCode::InvalidArgs)
    }
}

fn parse_chunk_interval_ns(args: &[Value]) -> Result<i64, ResultCode> {
    let interval = args
        .get(1)
        .and_then(Value::to_integer)
        .unwrap_or(60_000_000_000);
    if interval > 0 {
        Ok(interval)
    } else {
        Err(ResultCode::InvalidArgs)
    }
}

fn parse_row_args(args: &[Value]) -> Result<TtsHypertableRow, String> {
    let ts_ns = args
        .first()
        .and_then(Value::to_integer)
        .ok_or_else(|| "tts_hypertable requires ts_ns".to_string())?;
    let value_real = match args.get(1).map(Value::value_type) {
        Some(ValueType::Null) | None => None,
        _ => args.get(1).and_then(Value::to_float),
    };
    let quality = args.get(2).and_then(Value::to_integer).unwrap_or_default();

    Ok(TtsHypertableRow {
        rowid: 0,
        ts_ns,
        value_real,
        quality,
    })
}

fn ensure_storage(
    conn: &Arc<Connection>,
    table_name: &str,
    chunk_interval_ns: i64,
) -> Result<(), String> {
    if !schema_object_exists(conn, "table", "_tts_hypertables")? {
        conn.execute(
            "CREATE TABLE _tts_hypertables (
            hypertable_id INTEGER PRIMARY KEY AUTOINCREMENT,
            table_name TEXT NOT NULL,
            time_column TEXT NOT NULL DEFAULT 'ts_ns',
            chunk_interval_ns INTEGER NOT NULL,
            storage_layout TEXT NOT NULL DEFAULT 'columnar',
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE (table_name)
        )",
            &[],
        )
        .map_err(|code| code.to_string())?;
    }
    if !schema_object_exists(conn, "table", "_tts_hypertable_rows")? {
        conn.execute(
            "CREATE TABLE _tts_hypertable_rows (
            table_name TEXT NOT NULL,
            rowid INTEGER PRIMARY KEY AUTOINCREMENT,
            ts_ns INTEGER NOT NULL,
            value_real REAL,
            quality INTEGER NOT NULL DEFAULT 0
        )",
            &[],
        )
        .map_err(|code| code.to_string())?;
    }
    let values = vec![
        Value::from_text(table_name.to_string()),
        Value::from_integer(chunk_interval_ns),
        Value::from_text(table_name.to_string()),
    ];
    conn.execute(
        "UPDATE _tts_hypertables \
         SET time_column = 'ts_ns', chunk_interval_ns = ?, storage_layout = 'columnar' \
         WHERE table_name = ?",
        &[
            Value::from_integer(chunk_interval_ns),
            Value::from_text(table_name.to_string()),
        ],
    )
    .map_err(|code| code.to_string())?;
    conn.execute(
        "INSERT INTO _tts_hypertables (table_name, time_column, chunk_interval_ns, storage_layout) \
         SELECT ?, 'ts_ns', ?, 'columnar' \
         WHERE NOT EXISTS (SELECT 1 FROM _tts_hypertables WHERE table_name = ?)",
        &values,
    )
    .map_err(|code| code.to_string())?;
    Ok(())
}

fn schema_object_exists(
    conn: &Arc<Connection>,
    object_type: &str,
    name: &str,
) -> Result<bool, String> {
    let stmt = conn
        .prepare("SELECT 1 FROM sqlite_schema WHERE type = ? AND name = ?")
        .map_err(|code| code.to_string())?;
    stmt.bind_at(
        NonZeroUsize::new(1).expect("nonzero bind index"),
        Value::from_text(object_type.to_string()),
    );
    stmt.bind_at(
        NonZeroUsize::new(2).expect("nonzero bind index"),
        Value::from_text(name.to_string()),
    );
    match stmt.step() {
        StepResult::Row => Ok(true),
        StepResult::Done => Ok(false),
        StepResult::Interrupt => Err(ResultCode::Interrupt.to_string()),
        StepResult::Busy => Err(ResultCode::Busy.to_string()),
        StepResult::Error => Err(ResultCode::Error.to_string()),
    }
}

fn optional_float_value(value: Option<f64>) -> Value {
    match value {
        Some(value) => Value::from_float(value),
        None => Value::null(),
    }
}

fn trim_sql_quotes(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.len() >= 2 {
        let first = trimmed.as_bytes()[0];
        let last = trimmed.as_bytes()[trimmed.len() - 1];
        if matches!((first, last), (b'`', b'`') | (b'"', b'"') | (b'\'', b'\'')) {
            return trimmed[1..trimmed.len() - 1].to_string();
        }
    }
    trimmed.to_string()
}

fn is_simple_identifier(identifier: &str) -> bool {
    let mut chars = identifier.chars();
    match chars.next() {
        Some(c) if c == '_' || c.is_ascii_alphabetic() => {}
        _ => return false,
    }
    chars.all(|c| c == '_' || c.is_ascii_alphanumeric())
}
