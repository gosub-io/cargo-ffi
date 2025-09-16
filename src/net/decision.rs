use crate::engine::UaPolicy;
use crate::net::decision::sniff::ResponseClass;
use crate::net::decision::types::{DecisionOutcome, HandlingDecision, RenderTarget, RequestDestination};
use crate::net::types::FetchResultMeta;

mod sniff;
pub mod types;

pub fn decide_handling(
    _meta: &FetchResultMeta,
    _dest: RequestDestination,
    _peek: &[u8],
    _policy: &UaPolicy,
) -> DecisionOutcome {

    DecisionOutcome {
        class: ResponseClass::Html,         // pretend we classified it as HTML
        sniffed_class: None,                // no sniffing performed
        declared_mime: None,                // no declared mime
        disposition_attachment: false,      // not an attachment
        decision: HandlingDecision::Render(
            RenderTarget::HtmlParser        // force it into HTML parser
        ),
    }
}