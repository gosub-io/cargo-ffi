fn main() {
    println!("Hello, this is a Rust example using Gosub!");

    let mut engine = gosub_engine::Engine::new();
    engine.load_url("https://example.com");

    for _ in 0..10 {
        if engine.tick() {
            let fb = engine.render();
            println!("Rendered frame: {:?}", fb);
        }
    }
}