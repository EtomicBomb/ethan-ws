use html_escape::{encode_double_quoted_attribute_to_writer, encode_safe_to_writer};
use axum::response::{Html, IntoResponse, Response};
use std::io::Write as _;

#[derive(Clone, Default, Debug)]
pub struct HtmlBuf {
    buf: Vec<u8>,
    start_tag_unclosed: bool,
}

impl HtmlBuf {
    #[inline]
    pub fn node<F: FnOnce(Self) -> Self>(mut self, tag: &'static str, f: F) -> Self {
        self.close_start_tag();
        write!(self.buf, "<{}", tag).unwrap();
        self.start_tag_unclosed = true;
        self = f(self);
        self.close_start_tag();
        if !is_void(tag) {
            write!(self.buf, "</{}>", tag).unwrap();
        }
        self
    }

    #[inline]
    pub fn map<F: FnOnce(Self) -> Self>(self, f: F) -> Self {
        f(self)
    }

    #[inline]
    pub fn map_some<T, F>(self, element: Option<T>, f: F) -> Self 
    where F: FnOnce(Self, T) -> Self 
    {
        match element {
            Some(element) => f(self, element),
            None => self,
        }
    }

    #[inline]
    pub fn map_if<F: FnOnce(Self) -> Self>(self, cond: bool, f: F)-> Self {
        if cond {
            f(self)
        } else {
            self
        }
    }

    pub fn a<S: AsRef<str>>(mut self, attr: &'static str, value: S) -> Self {
        assert!(self.start_tag_unclosed, "no node to add the attribute to");
        write!(self.buf, " {}=\"", attr).unwrap();
        encode_double_quoted_attribute_to_writer(value.as_ref(), &mut self.buf).unwrap();
        write!(self.buf, "\"").unwrap();
        self
    }

    pub fn text<S: AsRef<str>>(mut self, text: S) -> Self {
        self.close_start_tag();
        encode_safe_to_writer(text.as_ref(), &mut self.buf).unwrap();
        self
    }

    fn close_start_tag(&mut self) {
        if self.start_tag_unclosed {
            write!(self.buf, ">").unwrap();
            self.start_tag_unclosed = false;
        }
    }
}

impl IntoResponse for HtmlBuf {
    fn into_response(self) -> Response {
        Html(self.buf).into_response()
    }
}

fn is_void(tag: &str) -> bool {
    // https://developer.mozilla.org/en-US/docs/Glossary/Void_element
    let void_elements = [
        "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param",
        "source", "track", "wbr",
    ];
    assert!(tag.bytes().all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'z' | b'-')));
    void_elements.binary_search(&tag).is_ok()
}


