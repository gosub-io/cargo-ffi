pub struct ResourceHint {
    pub url: Url,
    pub dest: RequestDestination,
    pub referrer: Option<Url>,
    pub cross_origin: bool,
    pub integrity: Option<String>,
    pub priority: Priority,
}

pub trait HtmlPipeline {
    fn parse_stream(
        &self,
        final_url: Url,
        body: Arc<SharedBody>
    ) -> impl Future<Output = anyhow::REsult<DummyDocument>> + Send;

    fn parse_bytes(
        &self,
        final_url: Url,
        bytes: &[u8],
    ) -> impl Future<Output = anyhow::Result<DummyDocument>> + Send;
}