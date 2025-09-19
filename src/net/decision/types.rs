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
    /// The coarse class of the response, based on sniffing and/or declared MIME type.
    pub class: ResponseClass,
    /// The coarse class of the response, based on sniffing only (if sniffing was performed).
    pub sniffed_class: Option<ResponseClass>,
    /// The declared MIME type from the `Content-Type` header, if any and parseable.
    pub declared_mime: Option<Mime>,
    /// Whether the response had a `Content-Disposition: attachment` header.
    pub disposition_attachment: bool,
    /// The final decision on how to handle the response.
    pub decision: HandlingDecision,
}

// Final decision for the response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HandlingDecision {
    /// Resource needs to be rendered based on its target (html parser, css parser, js engine, image decoder, etc).
    Render(RenderTarget),
    /// Resource should be downloaded to the given path.
    Download { path: PathBuf },
    /// Resource should be opened externally (e.g. PDF in external viewer).
    OpenExternal,
    /// Resource should be blocked for the given reason.
    Block(BlockReason),
    /// Resource should be cancelled (aborted silently).
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

impl std::fmt::Display for BlockReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BlockReason::TypeMismatch => write!(f, "type mismatch"),
            BlockReason::NosniffMismatch => write!(f, "nosniff mismatch"),
            BlockReason::TypeUnknown => write!(f, "unknown type"),
            BlockReason::Policy => write!(f, "policy block"),
        }
    }
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