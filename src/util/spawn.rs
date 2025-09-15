use tokio::task::{self, JoinHandle};

/// Spawns a name task on tokio runtime with the given name and future. Will return the join
/// handle
pub fn spawn_named<F, T>(name: &str, fut: F) -> JoinHandle<T>
where
    F: std::future::Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    task::Builder::new()
        .name(name)
        .spawn(fut)
        .expect(format!("failed to spawn task {}", name).as_str())
}
