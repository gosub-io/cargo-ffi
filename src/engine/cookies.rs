// src/engine/cookies.rs
//! Cookies: [`CookieJar`], [`CookieStore`] and backends.

mod cookies;
mod cookie_jar;
mod store;
mod persistent_cookie_jar;

pub use cookies::Cookie;
pub use cookies::CookieJarHandle;
pub(crate) use cookies::CookieStoreHandle;

pub use cookie_jar::CookieJar;
pub use cookie_jar::DefaultCookieJar;
pub use persistent_cookie_jar::PersistentCookieJar;

pub use store::CookieStore;
pub use store::JsonCookieStore;
pub use store::SqliteCookieStore;