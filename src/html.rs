mod parser;

pub use parser::{DummyDocument, DummyHtml5Config, DocumentError, ResourceHint, ResourceKind};
pub use parser::parse_main_document_stream;
