use html5ever::serialize::{self, TraversalScope};
use markup5ever_arcdom::SerializableHandle;

// Modified version of crabquery

use {
    html5ever::{
        driver::ParseOpts, parse_document, tendril::TendrilSink, tree_builder::TreeBuilderOpts,
    },
    markup5ever::{Attribute, QualName},
    markup5ever_arcdom::{ArcDom, Handle, NodeData},
    std::{cell::Ref, collections::HashMap, convert::TryFrom, sync::Arc},
};

pub struct Document {
    doc: ArcDom,
}

fn default_parse_opts() -> ParseOpts {
    ParseOpts {
        tree_builder: TreeBuilderOpts {
            drop_doctype: true,
            ..Default::default()
        },
        ..Default::default()
    }
}

impl TryFrom<&str> for Document {
    type Error = ao3fti_common::Report;

    fn try_from(input: &str) -> Result<Self, Self::Error> {
        let doc = parse_document(ArcDom::default(), default_parse_opts())
            .from_utf8()
            .read_from(&mut input.as_bytes())?;

        Ok(Self { doc })
    }
}

impl Document {
    pub fn select(&self, selector: impl Into<Selector>) -> Vec<Element> {
        let sel: Selector = selector.into();
        sel.find(self.doc.document.children.borrow())
    }
}

#[derive(Debug, PartialEq, Clone)]
enum AttributeSpec {
    Present,
    Exact(String),
    Starts(String),
    Ends(String),
    Contains(String),
}

impl AttributeSpec {
    fn matches(&self, other: String) -> bool {
        use AttributeSpec::*;

        match self {
            Present => true,
            Exact(v) => &other == v,
            Starts(v) => other.starts_with(v),
            Ends(v) => other.ends_with(v),
            Contains(v) => other.contains(v),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
struct Matcher {
    tag: Vec<String>,
    class: Vec<String>,
    id: Vec<String>,
    attribute: HashMap<String, AttributeSpec>,
    direct_match: bool,
}

impl From<String> for Matcher {
    fn from(input: String) -> Self {
        Self::from(input.as_str())
    }
}

impl From<&str> for Matcher {
    fn from(input: &str) -> Self {
        let mut segments = vec![];
        let mut buf = "".to_string();

        for c in input.chars() {
            match c {
                '>' => {
                    return Self {
                        tag: vec![],
                        class: vec![],
                        id: vec![],
                        attribute: HashMap::new(),
                        direct_match: true,
                    };
                }
                '#' | '.' | '[' => {
                    segments.push(buf);
                    buf = "".to_string();
                }
                ']' => {
                    segments.push(buf);
                    buf = "".to_string();
                    continue;
                }
                _ => {}
            };

            buf.push(c);
        }
        segments.push(buf);

        let mut res = Self {
            tag: vec![],
            class: vec![],
            id: vec![],
            attribute: HashMap::new(),
            direct_match: false,
        };

        for segment in segments {
            match segment.chars().next() {
                Some('#') => res.id.push(segment[1..].to_string()),
                Some('.') => res.class.push(segment[1..].to_string()),
                Some('[') => res.add_data_attribute(segment[1..].to_string()),
                None => {}
                _ => res.tag.push(segment),
            }
        }

        res
    }
}

impl Matcher {
    fn add_data_attribute(&mut self, spec: String) {
        use AttributeSpec::*;

        let parts = spec.split('=').collect::<Vec<_>>();

        if parts.len() == 1 {
            let k = parts[0];
            self.attribute.insert(k.to_string(), Present);
            return;
        }

        let v = parts[1].trim_matches('"').to_string();
        let k = parts[0];
        let k = k[..k.len() - 1].to_string();

        match parts[0].chars().last() {
            Some('^') => {
                self.attribute.insert(k, Starts(v));
            }
            Some('$') => {
                self.attribute.insert(k, Ends(v));
            }
            Some('*') => {
                self.attribute.insert(k, Contains(v));
            }
            Some(_) => {
                let k = parts[0].to_string();
                self.attribute.insert(k, Exact(v));
            }
            None => {
                panic!("Colud not parse attribute spec \"{}\"", spec);
            }
        }
    }

    fn matches(&self, name: &QualName, attrs: Ref<'_, Vec<Attribute>>) -> bool {
        let mut id_match = self.id.is_empty();
        if let Some(el_id) = get_attr(&attrs, "id") {
            let el_ids: Vec<_> = el_id.split_whitespace().collect();
            id_match = self.id.iter().all(|id| el_ids.iter().any(|eid| eid == id))
        }

        let mut class_match = self.class.is_empty();
        if let Some(el_class) = get_attr(&attrs, "class") {
            let el_classes: Vec<_> = el_class.split_whitespace().collect();

            class_match = self
                .class
                .iter()
                .all(|class| el_classes.iter().any(|eclass| eclass == class))
        }

        let mut attr_match = true;
        for (k, v) in &self.attribute {
            if let Some(value) = get_attr(&attrs, k.as_str()) {
                if !v.matches(value) {
                    attr_match = false;
                    break;
                }
            }
        }

        let name = name.local.to_string();
        let tag_match = self.tag.is_empty() || self.tag.iter().any(|tag| &name == tag);

        tag_match && id_match && class_match && attr_match
    }
}

#[derive(Debug, PartialEq)]
pub struct Selector {
    matchers: Vec<Matcher>,
}

impl From<&str> for Selector {
    fn from(input: &str) -> Self {
        let matchers: Vec<_> = input.split_whitespace().map(Matcher::from).collect();

        Selector { matchers }
    }
}

impl From<String> for Selector {
    fn from(input: String) -> Self {
        let matchers: Vec<_> = input.split_whitespace().map(Matcher::from).collect();

        Selector { matchers }
    }
}

fn get_attr(attrs: &Ref<'_, Vec<Attribute>>, name: &str) -> Option<String> {
    attrs
        .iter()
        .filter(|attr| &attr.name.local == name)
        .take(1)
        .map(|attr| attr.value.to_string())
        .collect::<Vec<_>>()
        .pop()
}

impl Selector {
    fn find_nodes(
        &self,
        matcher: &Matcher,
        elements: Vec<Handle>,
        direct_match: bool,
    ) -> Vec<Handle> {
        let mut acc = vec![];

        for el in elements.iter() {
            if !direct_match {
                let children: Vec<_> = el.children.borrow().iter().map(Arc::clone).collect();
                acc.append(&mut self.find_nodes(matcher, children, false));
            }

            match el.data {
                NodeData::Element {
                    ref name,
                    ref attrs,
                    ..
                } if matcher.matches(name, attrs.borrow()) => {
                    acc.push(Arc::clone(el));
                }
                _ => {}
            };
        }

        acc
    }

    fn find(&self, elements: Ref<'_, Vec<Handle>>) -> Vec<Element> {
        let mut elements: Vec<_> = elements.iter().map(Arc::clone).collect();
        let mut direct_match = false;

        for matcher in &self.matchers {
            if matcher.direct_match {
                direct_match = true;
                elements = elements
                    .iter()
                    .flat_map(|el| {
                        el.children
                            .borrow()
                            .iter()
                            .map(Arc::clone)
                            .collect::<Vec<_>>()
                    })
                    .collect();
                continue;
            }
            elements = self.find_nodes(matcher, elements, direct_match);
            direct_match = false;
        }

        elements.iter().map(Element::from).collect()
    }
}

#[derive(Debug)]
pub struct Element {
    handle: Handle,
}

impl From<Handle> for Element {
    fn from(e: Handle) -> Self {
        Self::from(&e)
    }
}

impl From<&Handle> for Element {
    fn from(e: &Handle) -> Self {
        Element {
            handle: Arc::clone(e),
        }
    }
}

impl Element {
    pub fn attr(&self, name: &str) -> Option<String> {
        match self.handle.data {
            NodeData::Element { ref attrs, .. } => get_attr(&attrs.borrow(), name),
            _ => None,
        }
    }

    pub fn text(&self) -> Option<String> {
        let mut res = "".to_string();
        let children = self.handle.children.borrow();

        for child in children.iter() {
            if let NodeData::Text { ref contents } = child.data {
                res.push_str(contents.borrow().to_string().as_str());
            } else {
                res.push_str(&(Element::from(child).text()?));
            }
        }

        Some(res)
    }

    pub fn inner_html(&self) -> Option<String> {
        let mut buf = Vec::new();

        let ser = serialize::serialize(
            &mut buf,
            &SerializableHandle::from(self.handle.clone()),
            serialize::SerializeOpts {
                traversal_scope: TraversalScope::ChildrenOnly(None),
                scripting_enabled: false,
                ..Default::default()
            },
        );

        if ser.is_err() {
            None
        } else {
            String::from_utf8(buf).ok()
        }
    }

    pub fn children(&self) -> Vec<Element> {
        self.handle
            .children
            .borrow()
            .iter()
            .filter(|n| matches!(n.data, NodeData::Element { .. }))
            .map(Element::from)
            .collect::<Vec<_>>()
    }

    pub fn select(&self, selector: impl Into<Selector>) -> Vec<Element> {
        let sel: Selector = selector.into();
        sel.find(self.handle.children.borrow())
    }
}
