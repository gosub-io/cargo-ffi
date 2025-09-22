use crate::engine::types::PeekBuf;
use crate::engine::UaPolicy;
use crate::net::decision::sniff::ResponseClass;
use crate::net::decision::types::{DecisionOutcome, HandlingDecision, RenderTarget, RequestDestination};
use crate::net::types::FetchResultMeta;

mod sniff;
pub mod types;

/// Decide how to handle a fetched response.
pub fn decide_handling(
    // The meta data from the request, including headers like content-type, no-sniff etc.
    _meta: &FetchResultMeta,
    // The request destination (e.g. "document", "script", "image", etc.)
    _dest: RequestDestination,
    // A peek buffer containing the first few bytes of the response body.
    _peek_buf: PeekBuf,
    // The user-agent policy, including settings like no-sniff, etc.
    _policy: &UaPolicy,
) -> DecisionOutcome {
    // @TODO: hardcoded for now
    DecisionOutcome {
        class: ResponseClass::Html,    // pretend we classified it as HTML
        sniffed_class: None,           // no sniffing performed
        declared_mime: None,           // no declared mime
        disposition_attachment: false, // not an attachment
        decision: HandlingDecision::Render(
            RenderTarget::HtmlParser, // force it into HTML parser
        ),
    }
}
