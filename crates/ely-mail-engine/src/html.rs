//! A mail's HTML as a tree, never run.

use std::borrow::Cow;
use std::cell::{Cell, RefCell};

use html5ever::{
    Attribute, LocalName, Namespace, ParseOpts, QualName, local_name, parse_document,
    tendril::{StrTendril, TendrilSink},
    tree_builder::{ElemName, ElementFlags, NodeOrText, QuirksMode, TreeBuilderOpts, TreeSink},
};

/// Deeper trees stall html5ever.
const DEPTH: usize = 512;
const CHUNK: usize = 4096;

/// The parsed tree, and whether it stopped short.
pub(crate) fn parse(html: &str) -> (Tree, bool) {
    let opts = ParseOpts {
        tree_builder: TreeBuilderOpts {
            // Mail never runs scripts.
            scripting_enabled: false,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut parser = parse_document(Tree::default(), opts);
    let mut rest = html;
    let mut short = false;
    while !rest.is_empty() {
        if parser.tokenizer.sink.sink.deep.get() {
            short = true;
            break;
        }
        let mut cut = rest.len().min(CHUNK);
        while !rest.is_char_boundary(cut) {
            cut += 1;
        }
        let (head, tail) = rest.split_at(cut);
        parser.process(StrTendril::from_slice(head));
        rest = tail;
    }
    (parser.finish(), short)
}

/// Visible HTML text, and whether it stopped short.
pub(crate) fn text(html: &str) -> (String, bool) {
    let (tree, short) = parse(html);
    (tree.text(), short)
}

pub(crate) enum Data {
    Document,
    Element {
        name: QualName,
        attrs: Vec<(LocalName, String)>,
        point: bool,
    },
    Text(String),
    Other,
}

pub(crate) struct Node {
    parent: Option<usize>,
    pub children: Vec<usize>,
    pub data: Data,
}

/// The tree html5ever's builder makes; the document is 0.
pub(crate) struct Tree {
    nodes: RefCell<Vec<Node>>,
    /// An element went past `DEPTH`.
    deep: Cell<bool>,
}

impl Default for Tree {
    fn default() -> Self {
        let document = Node {
            parent: None,
            children: Vec::new(),
            data: Data::Document,
        };
        Tree {
            nodes: RefCell::new(vec![document]),
            deep: Cell::new(false),
        }
    }
}

#[derive(Debug)]
pub(crate) struct Name(QualName);

impl ElemName for Name {
    fn ns(&self) -> &Namespace {
        &self.0.ns
    }

    fn local_name(&self) -> &LocalName {
        &self.0.local
    }
}

impl Tree {
    pub(crate) fn nodes(&self) -> std::cell::Ref<'_, Vec<Node>> {
        self.nodes.borrow()
    }

    fn add(&self, data: Data) -> usize {
        let mut nodes = self.nodes.borrow_mut();
        nodes.push(Node {
            parent: None,
            children: Vec::new(),
            data,
        });
        nodes.len() - 1
    }

    fn detach(&self, node: usize) {
        let mut nodes = self.nodes.borrow_mut();
        if let Some(parent) = nodes[node].parent.take() {
            nodes[parent].children.retain(|&child| child != node);
        }
    }

    /// Inserts a child at index `at`.
    fn insert(&self, parent: usize, at: usize, child: NodeOrText<usize>) {
        let node = match child {
            NodeOrText::AppendText(text) => {
                let mut nodes = self.nodes.borrow_mut();
                let before = at.checked_sub(1).map(|i| nodes[parent].children[i]);
                if let Some(Data::Text(held)) = before.map(|i| &mut nodes[i].data) {
                    held.push_str(&text);
                    return;
                }
                drop(nodes);
                self.add(Data::Text(text.to_string()))
            }
            NodeOrText::AppendNode(node) => {
                self.detach(node);
                node
            }
        };
        let mut nodes = self.nodes.borrow_mut();
        let at = at.min(nodes[parent].children.len());
        nodes[parent].children.insert(at, node);
        nodes[node].parent = Some(parent);
        if matches!(nodes[node].data, Data::Element { .. }) {
            let mut up = Some(parent);
            let mut depth = 0;
            while let Some(next) = up.filter(|_| depth <= DEPTH) {
                depth += 1;
                up = nodes[next].parent;
            }
            if depth > DEPTH {
                self.deep.set(true);
            }
        }
    }

    /// The visible text, read without recursion.
    fn text(&self) -> String {
        let nodes = self.nodes.borrow();
        let mut out = String::new();
        let mut stack = vec![0];
        while let Some(at) = stack.pop() {
            match &nodes[at].data {
                Data::Text(text) => out.push_str(text),
                Data::Element { name, .. } if hidden(&name.local) => out.push(' '),
                Data::Element { .. } | Data::Document => {
                    // Tags separate words.
                    out.push(' ');
                    stack.extend(nodes[at].children.iter().rev());
                }
                Data::Other => {}
            }
        }
        out
    }
}

/// Not drawn, per the HTML rendering rules.
pub(crate) fn hidden(name: &LocalName) -> bool {
    matches!(
        *name,
        local_name!("area")
            | local_name!("base")
            | local_name!("basefont")
            | local_name!("datalist")
            | local_name!("head")
            | local_name!("iframe")
            | local_name!("link")
            | local_name!("meta")
            | local_name!("noembed")
            | local_name!("noframes")
            | local_name!("param")
            | local_name!("rp")
            | local_name!("script")
            | local_name!("style")
            | local_name!("template")
            | local_name!("title")
    )
}

impl TreeSink for Tree {
    type Handle = usize;
    type Output = Self;
    type ElemName<'a> = Name;

    fn finish(self) -> Self {
        self
    }

    fn parse_error(&self, _msg: Cow<'static, str>) {}

    fn get_document(&self) -> usize {
        0
    }

    fn elem_name(&self, target: &usize) -> Name {
        match &self.nodes.borrow()[*target].data {
            Data::Element { name, .. } => Name(name.clone()),
            _ => unreachable!("html5ever asked a non-element for its name"),
        }
    }

    fn create_element(&self, name: QualName, attrs: Vec<Attribute>, flags: ElementFlags) -> usize {
        let point = flags.mathml_annotation_xml_integration_point;
        let attrs = attrs
            .into_iter()
            .map(|attr| (attr.name.local, attr.value.to_string()))
            .collect();
        let node = self.add(Data::Element { name, attrs, point });
        if flags.template {
            // Template contents stay out of the tree.
            self.add(Data::Other);
        }
        node
    }

    fn create_comment(&self, _text: StrTendril) -> usize {
        self.add(Data::Other)
    }

    fn create_pi(&self, _target: StrTendril, _data: StrTendril) -> usize {
        self.add(Data::Other)
    }

    fn append(&self, parent: &usize, child: NodeOrText<usize>) {
        let end = self.nodes.borrow()[*parent].children.len();
        self.insert(*parent, end, child);
    }

    fn append_based_on_parent_node(
        &self,
        element: &usize,
        prev_element: &usize,
        child: NodeOrText<usize>,
    ) {
        if self.nodes.borrow()[*element].parent.is_some() {
            self.append_before_sibling(element, child);
        } else {
            self.append(prev_element, child);
        }
    }

    fn append_doctype_to_document(&self, _: StrTendril, _: StrTendril, _: StrTendril) {}

    fn get_template_contents(&self, target: &usize) -> usize {
        target + 1
    }

    fn same_node(&self, x: &usize, y: &usize) -> bool {
        x == y
    }

    fn set_quirks_mode(&self, _mode: QuirksMode) {}

    fn append_before_sibling(&self, sibling: &usize, child: NodeOrText<usize>) {
        let (parent, at) = {
            let nodes = self.nodes.borrow();
            let parent = nodes[*sibling]
                .parent
                .expect("html5ever names a sibling with a parent");
            let at = nodes[parent].children.iter().position(|c| c == sibling);
            (parent, at.expect("a sibling among its parent's children"))
        };
        self.insert(parent, at, child);
    }

    fn add_attrs_if_missing(&self, target: &usize, added: Vec<Attribute>) {
        let mut nodes = self.nodes.borrow_mut();
        if let Data::Element { attrs, .. } = &mut nodes[*target].data {
            for attr in added {
                if !attrs.iter().any(|(name, _)| *name == attr.name.local) {
                    attrs.push((attr.name.local, attr.value.to_string()));
                }
            }
        }
    }

    fn remove_from_parent(&self, target: &usize) {
        self.detach(*target);
    }

    fn reparent_children(&self, node: &usize, new_parent: &usize) {
        let children = std::mem::take(&mut self.nodes.borrow_mut()[*node].children);
        for child in children {
            self.nodes.borrow_mut()[child].parent = None;
            self.append(new_parent, NodeOrText::AppendNode(child));
        }
    }

    fn is_mathml_annotation_xml_integration_point(&self, handle: &usize) -> bool {
        matches!(
            self.nodes.borrow()[*handle].data,
            Data::Element { point: true, .. }
        )
    }
}
