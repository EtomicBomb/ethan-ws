use {
    std::io::Write as _,
    std::fmt,
    axum::{body::Body},
};

#[derive(Clone, Default, Debug)]
#[must_use]
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

    pub fn a<S: AsRef<str>>(self, attribute: Attribute, value: S) -> Self {
        self.raw_attribute(attribute.into(), "", value)
    }

    #[inline]
    pub fn text<S: AsRef<str>>(mut self, text: S) -> Self {
        self.close_start_tag();
        html_escape::encode_safe_to_writer(text.as_ref(), &mut self.buf).unwrap();
        self
    }

    #[inline]
    fn raw_attribute<S: AsRef<str>>(mut self, attribute: &str, extension: &str, value: S) -> Self {
        assert!(self.start_tag_unclosed, "no node to add the attribute to");
        write!(self.buf, " {}{}=\"", attribute, extension).unwrap();
        html_escape::encode_double_quoted_attribute_to_writer(value.as_ref(), &mut self.buf).unwrap();
        write!(self.buf, "\"").unwrap();
        self
    }

    #[inline]
    fn close_start_tag(&mut self) {
        if self.start_tag_unclosed {
            write!(self.buf, ">").unwrap();
            self.start_tag_unclosed = false;
        }
    }

    pub fn hx_on<S: AsRef<str>>(self, event: &'static str, value: S) -> Self {
        self.raw_attribute("hx-on:", event, value)
    }

}

macro_rules! make_enum {
    ($enum_name:ident; $($name:ident $attribute_name:literal,)*) => {
        #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum $enum_name {
            $($name,)*
        }

        impl From<$enum_name> for &'static str {
            fn from(x: $enum_name) -> &'static str {
                match x {
                    $(Attribute::$name => $attribute_name,)*
                }

            }
        }

        impl fmt::Display for $enum_name {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str((*self).into())
            }
        }
    };
}


    // curl https://raw.githubusercontent.com/jozo/all-html-elements-and-attributes/master/html-elements-attributes.json | jq -r '.[] | .[]' | grep -v data | sort | uniq | awk '{a=$0; gsub("-", "_", a); print a, "\""  $0 "\""}'
    // https://htmx.org/reference/#attributes
make_enum! {
    Attribute;
    HxBoost "hx-boost",
    HxGet "hx-get",
    HxPost "hx-post",
    HxPushUrl "hx-push-url",
    HxSelect "hx-select",
    HxSelectOob "hx-select-oob",
    HxSwap "hx-swap",
    HxSwapOob "hx-swap-oob",
    HxTarget "hx-target",
    HxTrigger "hx-trigger",
    HxVals "hx-vals",
    HxConfirm "hx-confirm",
    HxDelete "hx-delete",
    HxDisable "hx-disable",
    HxDisabledElt "hx-disabled-elt",
    HxDisinherit "hx-disinherit",
    HxEncoding "hx-encoding",
    HxExt "hx-ext",
    HxHeaders "hx-headers",
    HxHistory "hx-history",
    HxHistoryElt "hx-history-elt",
    HxInclude "hx-include",
    HxIndicator "hx-indicator",
    HxParams "hx-params",
    HxPatch "hx-patch",
    HxPreserve "hx-preserve",
    HxPrompt "hx-prompt",
    HxPut "hx-put",
    HxReplaceUrl "hx-replace-url",
    HxRequest "hx-request",
    HxSse "hx-sse",
    HxSync "hx-sync",
    HxValidate "hx-validate",
    HxVars "hx-vars",
    HxWs "hx-ws",

    SseConnect "sse-connect",

    Accept "accept",
    AcceptCharset "accept-charset",
    Accesskey "accesskey",
    Action "action",
    Align "align",
    Allow "allow",
    Alt "alt",
    Async "async",
    Autocapitalize "autocapitalize",
    Autocomplete "autocomplete",
    Autofocus "autofocus",
    Autoplay "autoplay",
    Background "background",
    Bgcolor "bgcolor",
    Border "border",
    Buffered "buffered",
    Capture "capture",
    Challenge "challenge",
    Charset "charset",
    Checked "checked",
    Cite "cite",
    Class "class",
    Code "code",
    Codebase "codebase",
    Color "color",
    Cols "cols",
    Colspan "colspan",
    Content "content",
    Contenteditable "contenteditable",
    Contextmenu "contextmenu",
    Controls "controls",
    Coords "coords",
    Crossorigin "crossorigin",
    Csp "csp",
    Datetime "datetime",
    Decoding "decoding",
    Default "default",
    Defer "defer",
    Dir "dir",
    Dirname "dirname",
    Disabled "disabled",
    Download "download",
    Draggable "draggable",
    Enctype "enctype",
    Enterkeyhint "enterkeyhint",
    For "for",
    Form "form",
    Formaction "formaction",
    Formenctype "formenctype",
    Formmethod "formmethod",
    Formnovalidate "formnovalidate",
    Formtarget "formtarget",
    Headers "headers",
    Height "height",
    Hidden "hidden",
    High "high",
    Href "href",
    Hreflang "hreflang",
    HttpEquiv "http-equiv",
    Icon "icon",
    Id "id",
    Importance "importance",
    Inputmode "inputmode",
    Integrity "integrity",
    Intrinsicsize "intrinsicsize",
    Ismap "ismap",
    Itemprop "itemprop",
    Keytype "keytype",
    Kind "kind",
    Label "label",
    Lang "lang",
    Language "language",
    List "list",
    Loading "loading",
    Loop "loop",
    Low "low",
    Manifest "manifest",
    Max "max",
    Maxlength "maxlength",
    Media "media",
    Method "method",
    Min "min",
    Minlength "minlength",
    Multiple "multiple",
    Muted "muted",
    Name "name",
    Novalidate "novalidate",
    Open "open",
    Optimum "optimum",
    Pattern "pattern",
    Ping "ping",
    Placeholder "placeholder",
    Poster "poster",
    Preload "preload",
    Radiogroup "radiogroup",
    Readonly "readonly",
    Referrerpolicy "referrerpolicy",
    Rel "rel",
    Required "required",
    Reversed "reversed",
    Role "role",
    Rows "rows",
    Rowspan "rowspan",
    Sandbox "sandbox",
    Scope "scope",
    Scoped "scoped",
    Selected "selected",
    Shape "shape",
    Size "size",
    Sizes "sizes",
    Slot "slot",
    Span "span",
    Spellcheck "spellcheck",
    Src "src",
    Srcdoc "srcdoc",
    Srclang "srclang",
    Srcset "srcset",
    Start "start",
    Step "step",
    Style "style",
    Summary "summary",
    Tabindex "tabindex",
    Target "target",
    Title "title",
    Translate "translate",
    Type "type",
    Usemap "usemap",
    Value "value",
    Width "width",
    Wrap "wrap",
}

impl From<HtmlBuf> for Body {
    fn from(html: HtmlBuf) -> Self {
        Body::from(html.buf)
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

