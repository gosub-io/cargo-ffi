use std::path::PathBuf;
use mime::Mime;
use crate::net::decision::sniff::ResponseClass;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestDestination {
    MainDocument,
    Image,
    Style,
    Script,
    Font,
    Audio,
    Video,
    Worker,
    SharedWorker,
    ServiceWorker,
    Manifest,
    Track,
    Xslt,
    Fetch,
    Xhr,
    Other,
}

#[derive(Debug, Clone)]
pub struct DecisionOutcome {
    pub class: ResponseClass,
    pub sniffed_class: Option<ResponseClass>,
    pub declared_mime: Option<Mime>,
    pub disposition_attachment: bool,
    pub decision: HandlingDecision,
}

// Final decision for the response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HandlingDecision {
    Render(RenderTarget),
    Download { path: PathBuf },
    OpenExternal, // placeholder for integration
    Block(BlockReason),
    Cancel,
}

/// Why the response was blocked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockReason {
    /// The resource’s MIME type (declared/sniffed) is incompatible with the request destination.
    /// Example: `<img>` got back `text/html`.
    TypeMismatch,

    /// The response had `X-Content-Type-Options: nosniff`, and the declared MIME type
    /// was missing or not one of the allowed safe types for this destination.
    /// Example: `<script>` got back `text/plain; nosniff`.
    NosniffMismatch,

    /// The response MIME type was present but not recognized or supported by the engine.
    /// Example: `application/vnd.ms-excel` with no registered handler.
    TypeUnknown,

    /// A user agent or site policy explicitly forbids this load.
    /// Example: mixed-content block, CSP violation, or UA rule against auto-downloads.
    Policy,
}

// Where to send the stream if we render it inline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderTarget {
    HtmlParser,
    CssParser,
    JsEngine,
    ImageDecoder,
    MediaPipeline,
    FontLoader,
    PdfViewer,
    TextViewer,
    BodyToJs, // fetch/xhr -> JS
}