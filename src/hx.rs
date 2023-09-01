use {
    url::Url,
    http::header::{HeaderName, HeaderValue},
};

struct Boosted(bool);

struct CurrentUrl(Url);

struct HistoryRestoreRequest(bool);

struct Prompt(HeaderValue);

struct Request;

struct Target(HeaderValue);

struct TriggerName(HeaderValue);

struct Trigger<T>(T);

// response headers

struct Location(Url);

struct PushUrl(Url);

struct Trigger(HeaderValue);

struct TriggerAfterSettle(HeaderValue);

struct TriggerAfterSwap(HeaderValue);
