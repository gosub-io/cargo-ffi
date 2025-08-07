use std::any::Any;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use crate::zone::cookies::{CookieEntry, CookieJar};
use http::HeaderMap;
use url::Url;

// Default cookie jar which holds cookies for a single zone. It is in-memory only and does not do
// any persistance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefaultCookieJar {
    pub entries: HashMap<String, Vec<CookieEntry>>,
}

impl DefaultCookieJar {
    pub fn new() -> Self {
        dbg!("Creating DefaultCookieJar");
        DefaultCookieJar {
            entries: HashMap::new(),
        }
    }
}

impl CookieJar for DefaultCookieJar {
    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }

    fn store_response_cookies(&mut self, url: &Url, headers: &HeaderMap) {
        dbg!("[defaultcookiejar] Storing response cookies for URL: {}", url);
        let origin = url.origin().ascii_serialization();
        let _host = url.host_str().unwrap_or_default();
        let default_path = url.path().rsplit_once('/').map_or("/", |(a, _)| if a.is_empty() { "/" } else { a });

        let bucket = self.entries.entry(origin).or_default();

        for header in headers.get_all("set-cookie") {
            if let Ok(header_str) = header.to_str() {
                if let Some((name, rest)) = header_str.split_once('=') {
                    let mut cookie = CookieEntry {
                        name: name.trim().to_string(),
                        value: String::new(),
                        path: None,
                        domain: None,
                        secure: false,
                        expires: None,
                        same_site: None,
                        http_only: false,
                    };

                    for part in rest.split(';') {
                        let part = part.trim();
                        if cookie.value.is_empty() {
                            cookie.value = part.to_string();
                            continue;
                        }

                        if let Some((k, v)) = part.split_once('=') {
                            match k.to_ascii_lowercase().as_str() {
                                "path" => cookie.path = Some(v.to_string()),
                                "domain" => cookie.domain = Some(v.trim_start_matches('.').to_string()),
                                "expires" => cookie.expires = Some(v.to_string()),
                                _ => {}
                            }
                        } else if part.eq_ignore_ascii_case("secure") {
                            cookie.secure = true;
                        }
                    }

                    if cookie.path.is_none() {
                        cookie.path = Some(default_path.to_string());
                    }

                    // Replace existing cookie with same name
                    if let Some(existing) = bucket.iter_mut().find(|c| c.name == cookie.name) {
                        *existing = cookie;
                    } else {
                        bucket.push(cookie);
                    }
                }
            }
        }
    }

    fn get_request_cookies(&self, url: &Url) -> Option<String> {
        dbg!("[defaultcookiejar] Retrieving request cookies for URL: {}", url);

        let origin = url.origin().ascii_serialization();
        let host = url.host_str().unwrap_or_default();
        let path = url.path();
        let is_https = url.scheme() == "https";

        let cookies = self.entries.get(&origin)?;

        let header = cookies.iter().filter(|cookie| {
            // Check domain match
            match &cookie.domain {
                Some(domain) => {
                    host == domain || host.ends_with(&format!(".{}", domain))
                }
                None => true,
            }
        })
            .filter(|cookie| {
                // Check path match
                match &cookie.path {
                    Some(cookie_path) => path.starts_with(cookie_path),
                    None => true,
                }
            })
            .filter(|cookie| {
                // Check secure
                !cookie.secure || is_https
            })
            .map(|c| format!("{}={}", c.name, c.value))
            .collect::<Vec<_>>()
            .join("; ");

        if header.is_empty() {
            None
        } else {
            Some(header)
        }
    }

    fn clear(&mut self) {
        dbg!("[defaultcookiejar] Clearing cookie jar");
        self.entries.clear();
    }

    fn get_all_cookies(&self) -> Vec<(Url, String)> {
        dbg!("[defaultcookiejar] Retrieving all cookies");

        self.entries.iter().filter_map(|(origin, cookies)| {
            Url::parse(origin).ok().map(|url| {
                let str_ = cookies
                    .iter()
                    .map(|c| format!("{}={}", c.name, c.value))
                    .collect::<Vec<_>>()
                    .join("; ");
                (url, str_)
            })
        }).collect()
    }

    fn remove_cookie(&mut self, url: &Url, cookie_name: &str) {
        dbg!("[defaultcookiejar] Removing cookie '{}' for URL: {}", cookie_name, url);

        let origin = url.origin().ascii_serialization();
        if let Some(cookies) = self.entries.get_mut(&origin) {
            cookies.retain(|c| c.name != cookie_name);
        }
    }

    fn remove_cookies_for_url(&mut self, url: &Url) {
        dbg!("[defaultcookiejar] Removing all cookies for URL: {}", url);

        let origin = url.origin().ascii_serialization();
        self.entries.remove(&origin);
    }
}
