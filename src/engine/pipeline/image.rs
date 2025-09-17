use std::sync::Arc;
use crate::net::{stream_to_bytes, SharedBody};
use crate::net::types::FetchResultMeta;

pub trait ImagePipeline {
    async fn parse_stream(
        &self,
        meta: FetchResultMeta,
        peek: &[u8],
        body: Arc<SharedBody>
    ) -> anyhow::Result<image::DynamicImage>;

    async fn parse_bytes(
        &self,
        meta: FetchResultMeta,
        peek: &[u8],
        body: &[u8],
    ) -> anyhow::Result<image::DynamicImage>;
}


struct ImagePipelineImpl;

impl ImagePipeline for ImagePipelineImpl {
    async fn parse_stream(&self, meta: FetchResultMeta, peek: &[u8], shared: Arc<SharedBody>) -> anyhow::Result<image::DynamicImage> {
        // Normally, we send chunks to the Font parser. Right now, we just collect everything
        let b = stream_to_bytes(meta, peek.to_vec(), shared);
        Ok(String::from_utf8_lossy(b).to_string())
    }

    async fn parse_bytes(&self, meta: FetchResultMeta, peek: &[u8], body: &[u8]) -> anyhow::Result<image::DynamicImage>{
        let mut bytes = Vec::with_capacity(peek.len() + body.len());
        bytes.extend_from_slice(peek);
        bytes.extend_from_slice(body);

        Ok(String::from_utf8_lossy(bytes.as_slice()).to_string())
    }
}