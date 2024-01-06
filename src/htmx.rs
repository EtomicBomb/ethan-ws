use {
    std::io::Write as _,
    axum::response::{Html, IntoResponse, Response},
    html_escape::{encode_double_quoted_attribute_to_writer, encode_safe_to_writer},
};

macro_rules! attributes {
    ($($name:ident $attribute_name:literal,)*) => {
        $(
        #[allow(dead_code)]
        pub fn $name<S: AsRef<str>>(self, value: S) -> Self {
            self.attribute($attribute_name, value)
        }
        )*
    };
}

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
    pub fn chain<F: FnOnce(Self) -> Self>(self, f: F) -> Self {
        f(self)
    }

    #[inline]
    pub fn chain_if<F: FnOnce(Self) -> Self>(self, cond: bool, f: F) -> Self {
        if cond {
            f(self)
        } else {
            self
        }
    }

    #[inline]
    pub fn chain_if_some<T, F>(self, element: Option<T>, f: F) -> Self 
    where F: FnOnce(Self, T) -> Self 
    {
        match element {
            Some(element) => f(self, element),
            None => self,
        }
    }

    pub fn attribute<S: AsRef<str>>(self, attribute: &'static str, value: S) -> Self {
        self.raw_attribute(attribute, "", value)
    }

    pub fn hx_on<S: AsRef<str>>(self, event: &'static str, value: S) -> Self {
        self.raw_attribute("hx-on:", event, value)
    }

    attributes! {
        class "class",
        id "id",
        r#type "type",
        name "name",
        style "style",

        hx_boost "hx-boost",
        hx_get "hx-get",
        hx_post "hx-post",
//        hx_on "hx-on", // always concatenated with other stuff
        hx_push_url "hx-push_url",
        hx_select "hx-select",
        hx_select_oob "hx-select-oob",
        hx_swap "hx-swap",
        hx_swap_oob "hx-swap-oob",
        hx_target "hx-target",
        hx_trigger "hx-trigger",
        hx_vals "hx-vals",
        hx_confirm "hx-confirm",
        hx_delete "hx-delete",
        hx_disable "hx-disable",
        hx_disabled_elt "hx-disabled_elt",
        hx_disinherit "hx-disinherit",
        hx_encoding "hx-encoding",
        hx_ext "hx-ext",
        hx_headers "hx-headers",
        hx_history "hx-history",
        hx_history_elt "hx-history-elt",
        hx_include "hx-include",
        hx_indicator "hx-indicator",
        hx_params "hx-params",
        hx_patch "hx-patch",
        hx_preserve "hx-preserve",
        hx_prompt "hx-prompt",
        hx_put "hx-put",
        hx_replace_url "hx-replace-url",
        hx_request "hx-request",
        hx_sse "hx-sse",
        hx_sync "hx-sync",
        hx_validate "hx-validate",
        hx_vars "hx-vars",
        hx_ws "hx-ws",

        sse_connect "sse-connect",
    }

    pub fn text<S: AsRef<str>>(mut self, text: S) -> Self {
        self.close_start_tag();
        encode_safe_to_writer(text.as_ref(), &mut self.buf).unwrap();
        self
    }

    fn raw_attribute<S: AsRef<str>>(mut self, attribute: &str, extension: &str, value: S) -> Self {
        assert!(self.start_tag_unclosed, "no node to add the attribute to");
        write!(self.buf, " {}{}=\"", attribute, extension).unwrap();
        encode_double_quoted_attribute_to_writer(value.as_ref(), &mut self.buf).unwrap();
        write!(self.buf, "\"").unwrap();
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

macro_rules! uri_header {
    ($ident:ident $name:expr) => {
        pub struct $ident(pub Url);

        impl Header for $ident {
            fn name() -> &'static HeaderName {
                static NAME: HeaderName = HeaderName::from_static($name);
                &NAME
            }

            fn decode<'i, I>(values: &mut I) -> Result<Self, headers::Error>
            where
                I: Iterator<Item = &'i HeaderValue>,
            {
                let value = values
                    .last()
                    .ok_or_else(headers::Error::invalid)?
                    .to_str()
                    .map_err(|_| headers::Error::invalid())?
                    .parse()
                    .map_err(|_| headers::Error::invalid())?;
                Ok(Self(value))
            }

            fn encode<E>(&self, values: &mut E)
            where
                E: Extend<HeaderValue>,
            {
                let value = HeaderValue::try_from(self.0.as_str()).unwrap();
                values.extend(std::iter::once(value));
            }
        }
    };
}

macro_rules! raw_value_header {
    ($ident:ident $name:expr) => {
        pub struct $ident(pub HeaderValue);

        impl From<&'static str> for $ident {
            fn from(string: &'static str) -> Self {
                Self(HeaderValue::from_static(string))
            }
        }

        impl Header for $ident {
            fn name() -> &'static HeaderName {
                static NAME: HeaderName = HeaderName::from_static($name);
                &NAME
            }

            fn decode<'i, I>(values: &mut I) -> Result<Self, headers::Error>
            where
                I: Iterator<Item = &'i HeaderValue>,
            {
                let value = values
                    .last()
                    .ok_or_else(headers::Error::invalid)?
                    .clone();
                Ok(Self(value))
            }

            fn encode<E>(&self, values: &mut E)
            where
                E: Extend<HeaderValue>,
            {
                values.extend(std::iter::once(self.0.clone()));
            }
        }
    };
}

macro_rules! presence_header {
    ($ident:ident $name:expr) => {
        pub struct $ident;

        impl Header for $ident {
            fn name() -> &'static HeaderName {
                static NAME: HeaderName = HeaderName::from_static($name);
                &NAME
            }

            fn decode<'i, I>(values: &mut I) -> Result<Self, headers::Error>
            where
                I: Iterator<Item = &'i HeaderValue>,
            {
                let value = values.last().ok_or_else(headers::Error::invalid)?;
                if value != "true" {
                    return Err(headers::Error::invalid());
                }

                Ok(Self)
            }

            fn encode<E>(&self, values: &mut E)
            where
                E: Extend<HeaderValue>,
            {
                let value = HeaderValue::from_static("true");
                values.extend(std::iter::once(value));
            }
        }
    };
}

pub mod request {
    use http::header::{HeaderName, HeaderValue};
    use headers::{Header};
    use url::Url;
    use axum::response::{IntoResponseParts, ResponseParts};

    presence_header!(Boosted "hx-boosted");
    uri_header!(CurrentUrl "hx-current-url");
    presence_header!(HistoryRestore "hx-history-restore-request");
    raw_value_header!(Prompt "hx-prompt");
    presence_header!(Request "hx-request");
    raw_value_header!(Target "hx-target");
    raw_value_header!(TriggerName "hx-trigger-name");
    raw_value_header!(Trigger "hx-trigger");
}

pub mod response {
    pub use http::header::{HeaderName, HeaderValue};
    pub use headers::{Header};
    use url::Url;
    use axum::response::{IntoResponseParts, ResponseParts};
    
    uri_header!(Location "hx-location");
    uri_header!(PushUrl "hx-push-url");
    uri_header!(Redirect "hx-redirect");
    uri_header!(Refresh "hx-refresh");
    uri_header!(ReplaceUrl "hx-replace-url");
    raw_value_header!(Reswap "hx-reswap");
    raw_value_header!(Retarget "hx-retarget");
    raw_value_header!(Reselect "hx-reselect");
    raw_value_header!(Trigger "hx-trigger");
    raw_value_header!(TriggerAfterSettle "hx-trigger-after-settle");
    raw_value_header!(TriggerAfterSwap "hx-trigger-after-swap");
}

