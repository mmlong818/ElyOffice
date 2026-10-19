//! A table's borders, as Writer and Impress report them (DOC-07).

use std::io;

use serde_json::Value;

use crate::ipc::{Pack, Unpack};

/// One axis in twips: columns run across, rows down.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Lines {
    /// The first line, from `offset`.
    pub left: i32,
    /// The last line, from `offset`.
    pub right: i32,
    pub offset: i32,
    pub inner: Vec<Line>,
    /// Bounds of the first and last lines, when sent.
    pub ends: Option<[(i32, i32); 2]>,
}

/// An inner line and the kernel's bounds for it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Line {
    pub position: i32,
    pub min: i32,
    pub max: i32,
    pub hidden: bool,
}

/// The table holding the cursor.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Table {
    pub columns: Lines,
    pub rows: Lines,
}

/// Reads a `LOK_CALLBACK_TABLE_SELECTED` payload; `{}` is no table.
pub fn read(text: &str) -> Result<Option<Table>, String> {
    let fault = || format!("an unreadable table: {text}");
    let json: Value = serde_json::from_str(text).map_err(|_| fault())?;
    let object = json.as_object().ok_or_else(fault)?;
    if object.is_empty() {
        return Ok(None);
    }
    // Impress writes every value as text (boost's property tree).
    let int = |value: Option<&Value>| match value? {
        Value::Number(number) => i32::try_from(number.as_i64()?).ok(),
        Value::String(text) => text.parse().ok(),
        _ => None,
    };
    let flag = |value: Option<&Value>| match value? {
        Value::Bool(on) => Some(*on),
        Value::String(text) => text.parse().ok(),
        _ => None,
    };
    let lines = |key: &str| -> Option<Lines> {
        let axis = object.get(key)?;
        // Boost writes an empty list as an empty string.
        let entries = match axis.get("entries")? {
            Value::Array(entries) => entries.as_slice(),
            Value::String(text) if text.is_empty() => &[],
            _ => return None,
        };
        let inner = entries.iter().map(|entry| {
            Some(Line {
                position: int(entry.get("position"))?,
                min: int(entry.get("min"))?,
                max: int(entry.get("max"))?,
                hidden: flag(entry.get("hidden"))?,
            })
        });
        let bound = |key: &str| int(axis.get(key));
        let ends = match ["leftMin", "leftMax", "rightMin", "rightMax"].map(|key| axis.get(key).is_some()) {
            [false, false, false, false] => None,
            [true, true, true, true] => {
                Some([(bound("leftMin")?, bound("leftMax")?), (bound("rightMin")?, bound("rightMax")?)])
            }
            _ => return None,
        };
        Some(Lines {
            left: int(axis.get("left"))?,
            right: int(axis.get("right"))?,
            offset: int(axis.get("tableOffset"))?,
            inner: inner.collect::<Option<_>>()?,
            ends,
        })
    };
    let columns = lines("columns").ok_or_else(fault)?;
    let rows = lines("rows").ok_or_else(fault)?;
    Ok(Some(Table { columns, rows }))
}

impl Table {
    pub(crate) fn pack(table: Option<&Table>, pack: Pack) -> Pack {
        let Some(table) = table else {
            return pack.u64(0);
        };
        [&table.columns, &table.rows].into_iter().fold(pack.u64(1), |pack, axis| {
            let pack = pack
                .i64(axis.left.into())
                .i64(axis.right.into())
                .i64(axis.offset.into())
                .u64(axis.inner.len() as u64);
            let pack = match axis.ends {
                None => pack.u64(0),
                Some(ends) => ends.iter().fold(pack.u64(1), |pack, &(low, high)| pack.i64(low.into()).i64(high.into())),
            };
            axis.inner.iter().fold(pack, |pack, line| {
                pack.i64(line.position.into())
                    .i64(line.min.into())
                    .i64(line.max.into())
                    .u64(line.hidden as u64)
            })
        })
    }

    pub(crate) fn unpack(unpack: &mut Unpack) -> io::Result<Option<Table>> {
        if unpack.u64()? == 0 {
            return Ok(None);
        }
        let mut lines = || -> io::Result<Lines> {
            let mut int = || i32::try_from(unpack.i64()?).map_err(io::Error::other);
            let (left, right, offset) = (int()?, int()?, int()?);
            let count = unpack.u64()?;
            let ends = match unpack.u64()? {
                0 => None,
                _ => {
                    let mut int = || i32::try_from(unpack.i64()?).map_err(io::Error::other);
                    Some([(int()?, int()?), (int()?, int()?)])
                }
            };
            let inner = (0..count)
                .map(|_| {
                    let mut int = || i32::try_from(unpack.i64()?).map_err(io::Error::other);
                    let (position, min, max) = (int()?, int()?, int()?);
                    Ok(Line { position, min, max, hidden: unpack.u64()? != 0 })
                })
                .collect::<io::Result<_>>()?;
            Ok(Lines { left, right, offset, inner, ends })
        };
        let columns = lines()?;
        let rows = lines()?;
        Ok(Some(Table { columns, rows }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEEN: &str = r#"{ "columns": { "left": 0, "right": 9638, "tableOffset": 1135, "entries": [ { "position": 4819, "min": 300, "max": 9338, "hidden": false } ] }, "rows": { "left": 0, "right": 552, "tableOffset": 1418, "entries": [ { "position": 276, "min": 100, "max": 552, "hidden": false } ] } }"#;

    /// As Impress sent it for a new 2x2 table.
    const IMPRESS: &str = r#"{ "columns": { "tableOffset": "3929", "left": "0", "right": "7993", "entries": [ { "position": "3996", "min": "340", "max": "7653", "hidden": "false" } ] }, "rows": { "tableOffset": "4340", "left": "0", "right": "1091", "entries": [ { "position": "545", "min": "340", "max": "152183880", "hidden": "false" } ] }, "rectangle": { "x": "3940", "y": "4351", "width": "7993", "height": "1091" } }"#;

    #[test]
    fn impress_writes_numbers_as_text() {
        let table = read(IMPRESS).expect("readable").expect("a table");
        assert_eq!(table.columns.offset, 3929);
        assert_eq!(table.columns.inner, [Line { position: 3996, min: 340, max: 7653, hidden: false }]);
        assert_eq!(table.rows.right, 1091);
        assert!(read(&IMPRESS.replace("\"false\"", "\"no\"")).is_err(), "a flag is true or false");
        assert_eq!(table.rows.ends, None, "an unpatched kernel sends no ends");
    }

    /// One row, as Impress sends it with patch 0047.
    const ONE_ROW: &str = r#"{ "columns": { "tableOffset": "3929", "left": "0", "right": "7993", "leftMin": "-152183880", "leftMax": "7823", "rightMin": "170", "rightMax": "152183880", "entries": "" }, "rows": { "tableOffset": "4340", "left": "0", "right": "545", "leftMin": "-152183880", "leftMax": "28", "rightMin": "517", "rightMax": "152183880", "entries": "" } }"#;

    #[test]
    fn one_line_has_no_inner_lines_and_its_ends_cross_the_wire() {
        let table = read(ONE_ROW).expect("readable").expect("a table");
        assert!(table.columns.inner.is_empty() && table.rows.inner.is_empty());
        assert_eq!(table.rows.ends, Some([(-152_183_880, 28), (517, 152_183_880)]));
        assert!(read(&ONE_ROW.replace("\"entries\": \"\"", "\"entries\": \"x\"")).is_err(), "text is no list");
        assert!(read(&ONE_ROW.replace("\"leftMin\": \"-152183880\", ", "")).is_err(), "ends come in fours");
        let bytes = Table::pack(Some(&table), Pack::new()).bytes();
        assert_eq!(Table::unpack(&mut Unpack::new(&bytes)).expect("unpacks"), Some(table));
    }

    #[test]
    fn a_table_reads_and_crosses_the_wire() {
        assert_eq!(read("{ }"), Ok(None), "no table");
        let table = read(SEEN).expect("readable").expect("a table");
        assert_eq!(table.columns.offset, 1135);
        assert_eq!(table.rows.inner[0].position, 276);
        let text = SEEN.replace("\"left\": 0,", "\"left\": \"zero\",");
        assert!(read(&text).is_err(), "a word is no number");
        assert!(read("[]").is_err());
        let bytes = Table::pack(Some(&table), Pack::new()).bytes();
        let back = Table::unpack(&mut Unpack::new(&bytes)).expect("unpacks");
        assert_eq!(back, Some(table));
        let none = Table::pack(None, Pack::new()).bytes();
        assert_eq!(Table::unpack(&mut Unpack::new(&none)).expect("unpacks"), None);
    }
}
