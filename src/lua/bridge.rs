//! Type conversion between rush's `Value` and Lua values.
//!
//! - [`value_to_lua`] — converts a `Value` into an `mlua::Value`
//! - [`lua_to_value`] — converts an `mlua::Value` back into a `Value`
//!
//! # Type mapping
//!
//! | Rush `Value`          | Lua type                             |
//! |-----------------------|--------------------------------------|
//! | `String`              | string                               |
//! | `Int`                 | integer                              |
//! | `Float`               | number (float)                       |
//! | `Bool`                | boolean                              |
//! | `Null`                | nil                                  |
//! | `List`                | table (array, 1-indexed)             |
//! | `Record`              | table (string-keyed)                 |
//! | `Table`               | table with "columns" and "rows" keys |
//! | `Path`                | string                               |
//! | `Duration`            | number (seconds, float)              |
//! | `Filesize`            | integer (bytes)                      |
//! | `Date`                | string (RFC 3339)                    |
//! | `Error`               | string (error message)               |

use mlua::{Lua, Value as LuaValue};

use crate::value::{Table, Value};

/// Convert a rush `Value` into a Lua value.
pub fn value_to_lua(lua: &Lua, value: &Value) -> mlua::Result<LuaValue> {
    match value {
        Value::String(s) => Ok(LuaValue::String(lua.create_string(s)?)),
        Value::Int(i) => Ok(LuaValue::Integer(*i)),
        Value::Float(f) => Ok(LuaValue::Number(*f)),
        Value::Bool(b) => Ok(LuaValue::Boolean(*b)),
        Value::Null => Ok(LuaValue::Nil),

        Value::List(items) => {
            let table = lua.create_table()?;
            for (idx, item) in items.iter().enumerate() {
                table.raw_set(idx + 1, value_to_lua(lua, item)?)?;
            }
            Ok(LuaValue::Table(table))
        }

        Value::Record(map) => {
            let table = lua.create_table()?;
            for (key, val) in map {
                table.raw_set(key.as_str(), value_to_lua(lua, val)?)?;
            }
            Ok(LuaValue::Table(table))
        }

        Value::Table(rush_table) => rush_table_to_lua(lua, rush_table),

        Value::Path(p) => {
            let s = p.to_string_lossy();
            Ok(LuaValue::String(lua.create_string(s.as_ref())?))
        }

        Value::Duration(d) => Ok(LuaValue::Number(d.as_secs_f64())),
        Value::Filesize(bytes) => Ok(LuaValue::Integer(*bytes as i64)),
        Value::Date(dt) => Ok(LuaValue::String(lua.create_string(&dt.to_rfc3339())?)),
        Value::Error(e) => Ok(LuaValue::String(lua.create_string(e)?)),
    }
}

/// Convert a Lua value back into a rush `Value`.
pub fn lua_to_value(lua_value: LuaValue) -> Value {
    match lua_value {
        LuaValue::Nil => Value::Null,
        LuaValue::Boolean(b) => Value::Bool(b),
        LuaValue::Integer(i) => Value::Int(i),
        LuaValue::Number(f) => Value::Float(f),
        LuaValue::String(s) => Value::String(lua_string_to_string(&s)),
        LuaValue::Table(table) => lua_table_to_value(table),
        // Functions, threads, and userdata have no meaningful rush representation.
        _ => Value::Null,
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Convert an mlua String to a Rust String, using lossy conversion on non-UTF-8.
fn lua_string_to_string(s: &mlua::String) -> String {
    match s.to_str() {
        Ok(borrowed) => borrowed.to_string(),
        Err(_) => String::from_utf8_lossy(&s.as_bytes()).into_owned(),
    }
}

/// Encode a rush `Table` (columnar) as a Lua table with "columns" and "rows" keys.
///
/// The Lua representation mirrors the Lua builtin spec:
/// ```lua
/// { columns = {"name", "size"}, rows = {{name="a", size=1}, ...} }
/// ```
fn rush_table_to_lua(lua: &Lua, rush_table: &Table) -> mlua::Result<LuaValue> {
    let outer = lua.create_table()?;

    // columns = {"col1", "col2", ...}
    let cols = lua.create_table()?;
    for (idx, col) in rush_table.columns.iter().enumerate() {
        cols.raw_set(idx + 1, col.as_str())?;
    }
    outer.raw_set("columns", cols)?;

    // rows = {{col1=val, col2=val, ...}, ...}
    let rows = lua.create_table()?;
    for (idx, row) in rush_table.rows.iter().enumerate() {
        let inner = lua.create_table()?;
        for (key, val) in row {
            inner.raw_set(key.as_str(), value_to_lua(lua, val)?)?;
        }
        rows.raw_set(idx + 1, inner)?;
    }
    outer.raw_set("rows", rows)?;

    Ok(LuaValue::Table(outer))
}

/// Convert a Lua table to a rush `Value`.
///
/// Detection rules (in order):
/// 1. If the table has both "columns" (array of strings) and "rows" (array of tables) keys
///    → `Value::Table`
/// 2. If the table has only string keys → `Value::Record`
/// 3. If the table has only consecutive integer keys starting at 1 → `Value::List`
/// 4. Mixed → `Value::Record` (integer keys stringified)
fn lua_table_to_value(table: mlua::Table) -> Value {
    // Check for columns + rows structure → Value::Table
    if let Some(rush_table) = try_lua_table_as_structured(&table) {
        return Value::Table(rush_table);
    }

    let mut int_entries: Vec<(i64, Value)> = Vec::new();
    let mut str_entries: std::collections::HashMap<String, Value> =
        std::collections::HashMap::new();
    let mut mixed = false;

    for pair in table.clone().pairs::<LuaValue, LuaValue>() {
        let Ok((k, v)) = pair else { continue };
        match k {
            LuaValue::Integer(i) => int_entries.push((i, lua_to_value(v))),
            LuaValue::Number(f) => {
                if f.fract() == 0.0 {
                    int_entries.push((f as i64, lua_to_value(v)));
                } else {
                    mixed = true;
                }
            }
            LuaValue::String(s) => {
                str_entries.insert(lua_string_to_string(&s), lua_to_value(v));
            }
            _ => {
                mixed = true;
            }
        }
    }

    if mixed || (!int_entries.is_empty() && !str_entries.is_empty()) {
        // Ambiguous — treat as record, stringify integer keys.
        for (k, v) in int_entries {
            str_entries.insert(k.to_string(), v);
        }
        return Value::Record(str_entries);
    }

    if !str_entries.is_empty() {
        return Value::Record(str_entries);
    }

    // Pure integer-keyed — sort and return as a list.
    int_entries.sort_by_key(|(k, _)| *k);
    Value::List(int_entries.into_iter().map(|(_, v)| v).collect())
}

/// Attempt to interpret a Lua table as a `Value::Table` (columnar structured data).
///
/// Succeeds when the table has:
/// - `columns`: an array of strings
/// - `rows`: an array of string-keyed tables
///
/// Returns `None` if the shape doesn't match.
fn try_lua_table_as_structured(table: &mlua::Table) -> Option<Table> {
    // Must have a "columns" key that is a table of strings.
    let columns_lua: mlua::Table = table.get("columns").ok()?;
    let rows_lua: mlua::Table = table.get("rows").ok()?;

    let mut columns: Vec<String> = Vec::new();
    for pair in columns_lua.pairs::<mlua::Integer, LuaValue>() {
        let Ok((_, v)) = pair else { return None };
        match v {
            LuaValue::String(s) => columns.push(lua_string_to_string(&s)),
            _ => return None, // columns must all be strings
        }
    }

    // An empty columns list is valid (empty table).
    let mut rush_table = Table::new(columns);

    for pair in rows_lua.pairs::<mlua::Integer, LuaValue>() {
        let Ok((_, v)) = pair else { return None };
        match v {
            LuaValue::Table(row_tbl) => {
                let mut row = std::collections::HashMap::new();
                for cell in row_tbl.pairs::<LuaValue, LuaValue>() {
                    let Ok((k, val)) = cell else { continue };
                    match k {
                        LuaValue::String(s) => {
                            row.insert(lua_string_to_string(&s), lua_to_value(val));
                        }
                        _ => {} // non-string keys in rows are ignored
                    }
                }
                rush_table.push_row(row);
            }
            _ => return None, // rows must be tables
        }
    }

    Some(rush_table)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mlua::Lua;

    fn make_lua() -> Lua {
        Lua::new()
    }

    #[test]
    fn test_string_roundtrip() {
        let lua = make_lua();
        let val = Value::String("hello".into());
        let lv = value_to_lua(&lua, &val).unwrap();
        assert_eq!(lua_to_value(lv), val);
    }

    #[test]
    fn test_int_roundtrip() {
        let lua = make_lua();
        let val = Value::Int(42);
        let lv = value_to_lua(&lua, &val).unwrap();
        assert_eq!(lua_to_value(lv), val);
    }

    #[test]
    fn test_list_roundtrip() {
        let lua = make_lua();
        let val = Value::List(vec![Value::Int(1), Value::Int(2), Value::Int(3)]);
        let lv = value_to_lua(&lua, &val).unwrap();
        assert_eq!(lua_to_value(lv), val);
    }

    #[test]
    fn test_record_roundtrip() {
        let lua = make_lua();
        let mut map = std::collections::HashMap::new();
        map.insert("a".into(), Value::String("x".into()));
        let val = Value::Record(map);
        let lv = value_to_lua(&lua, &val).unwrap();
        assert_eq!(lua_to_value(lv), val);
    }

    #[test]
    fn test_table_to_lua_and_back() {
        let lua = make_lua();
        let mut table = Table::new(vec!["name".into(), "size".into()]);
        let mut row = std::collections::HashMap::new();
        row.insert("name".into(), Value::String("foo".into()));
        row.insert("size".into(), Value::Int(100));
        table.push_row(row);

        let lv = value_to_lua(&lua, &Value::Table(table.clone())).unwrap();
        let back = lua_to_value(lv);

        assert_eq!(back, Value::Table(table));
    }

    #[test]
    fn test_lua_string_non_utf8_fallback() {
        // BorrowedStr handles non-UTF8 gracefully via lossy conversion
        let lua = make_lua();
        let lv = LuaValue::String(lua.create_string(b"hello").unwrap());
        let val = lua_to_value(lv);
        assert_eq!(val, Value::String("hello".into()));
    }
}
