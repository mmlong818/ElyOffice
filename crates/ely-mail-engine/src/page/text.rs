//! A page as plain lines, for quoting it (C12.12).

use super::{Block, Page};

impl Page {
    /// Its words as lines; blank lines part the blocks.
    pub fn text(&self) -> String {
        lines(&self.blocks).join("\n")
    }
}

fn lines(blocks: &[Block]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for block in blocks {
        let next = block_lines(block);
        if next.is_empty() {
            continue;
        }
        if !out.is_empty() {
            out.push(String::new());
        }
        out.extend(next);
    }
    out
}

fn block_lines(block: &Block) -> Vec<String> {
    match block {
        Block::Text(inline) | Block::Heading(_, inline) => split(&inline.text),
        Block::Code(text) => split(text),
        Block::Rule => vec!["---".to_owned()],
        Block::Image(image) => match image.alt.trim() {
            "" => Vec::new(),
            alt => vec![format!("[{alt}]")],
        },
        Block::Quote(inner) => lines(inner)
            .into_iter()
            .map(|line| match line.is_empty() {
                true => ">".to_owned(),
                false => format!("> {line}"),
            })
            .collect(),
        Block::List { start, items } => {
            let mut out = Vec::new();
            for (at, item) in items.iter().enumerate() {
                let marker = match start {
                    Some(first) => format!("{}. ", first + at as u64),
                    None => "- ".to_owned(),
                };
                let pad = " ".repeat(marker.chars().count());
                for (row, line) in lines(item).into_iter().enumerate() {
                    out.push(match (row, line.is_empty()) {
                        (0, _) => format!("{marker}{line}"),
                        (_, true) => String::new(),
                        (_, false) => format!("{pad}{line}"),
                    });
                }
            }
            out
        }
    }
}

fn split(text: &str) -> Vec<String> {
    text.lines()
        .map(|line| line.trim_end().to_owned())
        .collect()
}
