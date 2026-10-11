//! What a spreadsheet reports about the current cell (TABLE-02, 03).

use serde_json::Value;

use super::Rect;
use crate::ipc::{Pack, Unpack};

/// What a spreadsheet reports about the current cell.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Cell {
    /// The cell cursor in twips; None while it hides.
    pub cursor: Option<Rect>,
    /// The cell's content as the formula bar shows it.
    pub formula: String,
    /// The selection's address, as "A1" or "A1:B3".
    pub address: String,
    /// Bumps when sheets switch or the sheet size changes.
    pub sheets: u64,
    /// Bumps when row or column headers change.
    pub headers: u64,
    /// The references of the formula being typed.
    pub marks: Vec<Mark>,
    /// Functions that fit the name being typed.
    pub functions: Vec<Function>,
    /// The call being typed: its signature and the argument.
    pub usage: String,
    /// Bumps on each request.
    pub requests: u64,
    /// The last thing the kernel asked Ely to show.
    pub request: Option<Request>,
}

/// What the kernel asks Ely to show (TABLE-08, 09).
#[derive(Clone, Debug, PartialEq)]
pub enum Request {
    /// The filter menu of a column's button.
    Filter { column: u32, row: u32 },
    /// The validation list to choose from.
    Choices { column: u32, row: u32 },
    /// An entry the validation refused or warned of.
    Invalid {
        title: String,
        message: String,
        /// stop, warning or info.
        style: String,
        accepted: bool,
    },
}

/// One reference of a formula being typed.
#[derive(Clone, Debug, PartialEq)]
pub struct Mark {
    pub rect: Rect,
    /// 0xRRGGBB, as the kernel colors the reference.
    pub color: u32,
    pub part: usize,
}

/// A function the kernel offers for a typed name.
#[derive(Clone, Debug, PartialEq)]
pub struct Function {
    pub name: String,
    pub signature: String,
    pub description: String,
}

impl Request {
    /// The request's kind, for logs without document text.
    pub fn kind(&self) -> &'static str {
        match self {
            Request::Filter { .. } => "a filter menu",
            Request::Choices { .. } => "a choice list",
            Request::Invalid { .. } => "an entry notice",
        }
    }
}

impl Cell {
    pub(crate) fn pack(&self, pack: Pack) -> Pack {
        let (x, y, w, h) = self.cursor.unwrap_or((-1, -1, -1, -1));
        let pack = pack
            .i64(x.into())
            .i64(y.into())
            .i64(w.into())
            .i64(h.into())
            .str(&self.formula)
            .str(&self.address)
            .u64(self.sheets)
            .u64(self.headers)
            .u64(self.marks.len() as u64);
        let pack = self.marks.iter().fold(pack, |pack, mark| {
            let (x, y, w, h) = mark.rect;
            pack.i64(x.into())
                .i64(y.into())
                .i64(w.into())
                .i64(h.into())
                .u64(mark.color.into())
                .u64(mark.part as u64)
        });
        let pack = pack.u64(self.functions.len() as u64);
        let pack = self
            .functions
            .iter()
            .fold(pack, |pack, function| {
                pack.str(&function.name)
                    .str(&function.signature)
                    .str(&function.description)
            })
            .str(&self.usage)
            .u64(self.requests);
        match &self.request {
            None => pack.u64(0),
            Some(Request::Filter { column, row }) => {
                pack.u64(1).u64((*column).into()).u64((*row).into())
            }
            Some(Request::Choices { column, row }) => {
                pack.u64(2).u64((*column).into()).u64((*row).into())
            }
            Some(Request::Invalid {
                title,
                message,
                style,
                accepted,
            }) => pack
                .u64(3)
                .str(title)
                .str(message)
                .str(style)
                .u64((*accepted).into()),
        }
    }

    pub(crate) fn unpack(unpack: &mut Unpack) -> std::io::Result<Cell> {
        fn number(unpack: &mut Unpack) -> std::io::Result<i32> {
            i32::try_from(unpack.i64()?).map_err(std::io::Error::other)
        }
        fn rect(unpack: &mut Unpack) -> std::io::Result<Rect> {
            Ok((
                number(unpack)?,
                number(unpack)?,
                number(unpack)?,
                number(unpack)?,
            ))
        }
        let cursor = rect(unpack)?;
        let (formula, address) = (unpack.str()?, unpack.str()?);
        let (sheets, headers) = (unpack.u64()?, unpack.u64()?);
        let marks = (0..unpack.u64()?)
            .map(|_| {
                Ok(Mark {
                    rect: rect(unpack)?,
                    color: u32::try_from(unpack.u64()?).map_err(std::io::Error::other)?,
                    part: unpack.u64()? as usize,
                })
            })
            .collect::<std::io::Result<_>>()?;
        let functions = (0..unpack.u64()?)
            .map(|_| {
                Ok(Function {
                    name: unpack.str()?,
                    signature: unpack.str()?,
                    description: unpack.str()?,
                })
            })
            .collect::<std::io::Result<_>>()?;
        let usage = unpack.str()?;
        let requests = unpack.u64()?;
        let index = |unpack: &mut Unpack| -> std::io::Result<u32> {
            u32::try_from(unpack.u64()?).map_err(std::io::Error::other)
        };
        let request = match unpack.u64()? {
            0 => None,
            1 => Some(Request::Filter {
                column: index(unpack)?,
                row: index(unpack)?,
            }),
            2 => Some(Request::Choices {
                column: index(unpack)?,
                row: index(unpack)?,
            }),
            3 => Some(Request::Invalid {
                title: unpack.str()?,
                message: unpack.str()?,
                style: unpack.str()?,
                accepted: unpack.u64()? != 0,
            }),
            other => return Err(std::io::Error::other(format!("no request {other}"))),
        };
        Ok(Cell {
            cursor: (cursor.2 >= 0).then_some(cursor),
            formula,
            address,
            sheets,
            headers,
            marks,
            functions,
            usage,
            requests,
            request,
        })
    }
}

/// A REFERENCE_MARKS payload.
pub fn marks_of(text: &str) -> Option<Vec<Mark>> {
    let json: Value = serde_json::from_str(text).ok()?;
    json["marks"]
        .as_array()?
        .iter()
        .map(|mark| {
            Some(Mark {
                rect: super::parse::rect_of(mark["rectangle"].as_str()?).ok()??,
                color: u32::from_str_radix(mark["color"].as_str()?, 16).ok()?,
                part: mark["part"].as_str()?.parse().ok()?,
            })
        })
        .collect()
}

/// A CALC_FUNCTION_LIST payload; "hidetip" is empty.
pub fn functions_of(text: &str) -> Option<Vec<Function>> {
    if text == "hidetip" {
        return Some(Vec::new());
    }
    let json: Value = serde_json::from_str(text).ok()?;
    json.as_array()?
        .iter()
        .map(|entry| {
            let signature = entry["signature"].as_str()?.to_string();
            let name = signature.split('(').next()?.trim().to_string();
            Some(Function {
                name,
                signature,
                description: entry["description"].as_str()?.to_string(),
            })
        })
        .collect()
}

/// A TOOLTIP payload's usage text, if it is one.
pub fn usage_of(text: &str) -> Option<String> {
    let json: Value = serde_json::from_str(text).ok()?;
    (json["type"] == "formulausage").then(|| json["text"].as_str().map(str::to_string))?
}

/// A TOOLTIP payload asking for a menu or a notice.
pub fn request_of(text: &str) -> Option<Request> {
    let json: Value = serde_json::from_str(text).ok()?;
    let index = |key: &str| u32::try_from(json[key].as_i64()?).ok();
    let text = |key: &str| json[key].as_str().map(str::to_string);
    match json["type"].as_str()? {
        "autofilter" => Some(Request::Filter {
            column: index("col")?,
            row: index("row")?,
        }),
        "datalist" => Some(Request::Choices {
            column: index("col")?,
            row: index("row")?,
        }),
        "validityerror" => Some(Request::Invalid {
            title: text("title")?,
            message: text("message")?,
            style: text("style")?,
            accepted: json["accepted"].as_bool()?,
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calc_formula_hints_read_out() {
        let marks = r#"{ "marks": [ { "rectangle": "1280, 512, 1279, 255", "color": "ef0fff", "part": "0" } ] }"#;
        assert_eq!(
            marks_of(marks),
            Some(vec![Mark {
                rect: (1280, 512, 1279, 255),
                color: 0xef0fff,
                part: 0
            }])
        );
        assert_eq!(marks_of(r#"{ "marks": [  ] }"#), Some(Vec::new()));
        let list = r#"[ {"index": 4, "signature": "SUM( Number 1, Number 2, ... )", "description": "Returns the sum of all arguments.", "namedRange": false } ]"#;
        let functions = functions_of(list).expect("a list");
        assert_eq!(functions[0].name, "SUM");
        assert_eq!(functions_of("hidetip"), Some(Vec::new()));
        assert_eq!(
            request_of(r#"{"type":"autofilter","col":2,"row":0}"#),
            Some(Request::Filter { column: 2, row: 0 })
        );
        let refused = r#"{"type":"validityerror","title":"Bad","message":"No","style":"stop","accepted":false}"#;
        assert!(matches!(
            request_of(refused),
            Some(Request::Invalid {
                accepted: false,
                ..
            })
        ));
        assert_eq!(
            request_of(r#"{"type":"autofilter","col":-1,"row":0}"#),
            None
        );
        let usage = r#"{ "type": "formulausage", "text": "SUM(► Number 1) : The numbers."}"#;
        assert_eq!(
            usage_of(usage).as_deref(),
            Some("SUM(► Number 1) : The numbers.")
        );
        assert_eq!(usage_of(r#"{ "text": "Author: Ada" }"#), None);
    }

    #[test]
    fn a_cell_packs_and_unpacks() {
        let cell = Cell {
            cursor: Some((1, 2, 3, 4)),
            formula: "=SUM(B3".into(),
            address: "A2".into(),
            sheets: 1,
            headers: 2,
            marks: vec![Mark {
                rect: (5, 6, 7, 8),
                color: 0xff0000,
                part: 1,
            }],
            functions: vec![Function {
                name: "SUM".into(),
                signature: "SUM()".into(),
                description: "Adds.".into(),
            }],
            usage: "SUM(►)".into(),
            requests: 3,
            request: Some(Request::Invalid {
                title: "Bad".into(),
                message: "No".into(),
                style: "stop".into(),
                accepted: false,
            }),
        };
        let bytes = cell.pack(Pack::new()).bytes();
        assert_eq!(
            Cell::unpack(&mut Unpack::new(&bytes)).expect("unpacks"),
            cell
        );
    }
}
