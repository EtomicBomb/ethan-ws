use {
    headers::Header,
    http::header::{HeaderName, HeaderValue},
    url::Url,
};

macro_rules! uri_header {
    ($ident:ident, $name:expr) => {
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
                let Ok(value) = HeaderValue::try_from(self.0.as_str()) else {
                    return;
                };
                values.extend(std::iter::once(value));
            }
        }
    };
}

//macro_rules! presence_header {
//    ($ident:ident, $name:expr) => {
//        pub struct $ident;
//
//        impl Header for $ident {
//            fn name() -> &'static HeaderName {
//                static NAME: HeaderName = HeaderName::from_static($name);
//                &NAME
//            }
//
//            fn decode<'i, I>(values: &mut I) -> Result<Self, headers::Error>
//            where
//                I: Iterator<Item = &'i HeaderValue>,
//            {
//                let value = values.last().ok_or_else(headers::Error::invalid)?;
//                if value != "true" {
//                    return Err(headers::Error::invalid());
//                }
//
//                Ok(Self)
//            }
//
//            fn encode<E>(&self, values: &mut E)
//            where
//                E: Extend<HeaderValue>,
//            {
//                let value = HeaderValue::from_static("true");
//                values.extend(std::iter::once(value));
//            }
//        }
//    };
//}

//pub struct UserInput;

//pub struct HtmlElementName;

//pub struct HtmlElementId;

//presence_header!(Boosted, "hx-boosted");

uri_header!(CurrentUrl, "hx-current-url");

//presence_header!(HistoryRestoreRequest, "hx-history-restore-request");

//pub struct Prompt(pub UserInput);

//presence_header!(Request, "hx-request");

//pub struct Target(pub HtmlElementId);

//pub struct TriggerName(pub HtmlElementName);

//pub struct TriggerRequest(pub HtmlElementId);

//uri_header!(Location, "hx-location");

//uri_header!(PushUrl, "hx-push-url");

//uri_header!(Redirect, "hx-redirect");

//uri_header!(Refresh, "hx-refresh");

uri_header!(ReplaceUrl, "hx-replace-url");

//pub struct Reswap(pub HeaderValue);

//pub struct Retarget(pub HeaderValue);

//pub struct Reselect(pub HeaderValue);

//pub struct TriggerResponse(pub HeaderValue);

//pub struct TriggerAfterSettle(pub HeaderValue);

//pub struct TriggerAfterSwap(pub HeaderValue);
