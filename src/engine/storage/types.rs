use std::fmt;
use url::Url;

/// (scheme, host, port) as a normalized string. Keep simple for now.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Origin(pub String);

impl fmt::Display for Origin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Partitioning key (future-proof for state partitioning).
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum PartitionKey {
    None,
    TopLevel(Origin),
}

impl Default for PartitionKey {
    fn default() -> Self { PartitionKey::None }
}

#[derive(Clone, Copy, Debug)]
pub enum PartitionPolicy { None, TopLevelOrigin }

pub fn compute_partition_key(u: &Url, p: PartitionPolicy) -> PartitionKey {
    match p {
        PartitionPolicy::None => PartitionKey::None,
        PartitionPolicy::TopLevelOrigin => {
            let o = Origin(u.origin().ascii_serialization());
            PartitionKey::TopLevel(o)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use url::Url;

    #[test]
    fn origin_display_roundtrips_inner_string() {
        let o = Origin("https://example.com".into());
        assert_eq!(o.to_string(), "https://example.com");
    }

    #[test]
    fn partitionkey_default_is_none() {
        let pk: PartitionKey = Default::default();
        assert_eq!(pk, PartitionKey::None);
    }

    #[test]
    fn compute_none_policy_returns_none() {
        let u = Url::parse("https://example.com/path?q=1#frag").unwrap();
        assert_eq!(compute_partition_key(&u, PartitionPolicy::None), PartitionKey::None);
    }

    #[test]
    fn compute_toplevel_uses_origin_ascii_serialization_with_non_default_port() {
        let u = Url::parse("https://sub.example.com:8443/path?q=1#f").unwrap();
        let pk = compute_partition_key(&u, PartitionPolicy::TopLevelOrigin);
        match pk {
            PartitionKey::TopLevel(Origin(s)) => {
                assert_eq!(s, "https://sub.example.com:8443");
            }
            _ => panic!("expected TopLevel origin"),
        }
    }

    #[test]
    fn compute_toplevel_elides_default_port() {
        // For HTTPS default port (443), ascii_serialization omits the port.
        let u = Url::parse("https://example.com/anything").unwrap();
        let pk = compute_partition_key(&u, PartitionPolicy::TopLevelOrigin);
        assert_eq!(pk, PartitionKey::TopLevel(Origin("https://example.com".into())));
    }

    #[test]
    fn compute_toplevel_ipv6_with_port() {
        let u = Url::parse("http://[2001:db8::1]:8080/").unwrap();
        let pk = compute_partition_key(&u, PartitionPolicy::TopLevelOrigin);
        assert_eq!(pk, PartitionKey::TopLevel(Origin("http://[2001:db8::1]:8080".into())));
    }

    #[test]
    fn partitionkey_equality_and_hash_semantics() {
        use std::collections::HashSet;

        let a = PartitionKey::TopLevel(Origin("https://a.example".into()));
        let b = PartitionKey::TopLevel(Origin("https://a.example".into()));
        let c = PartitionKey::TopLevel(Origin("https://b.example".into()));
        let none = PartitionKey::None;

        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_ne!(a, none);

        let mut set = HashSet::new();
        set.insert(a.clone());
        set.insert(b.clone()); // should not create a duplicate
        set.insert(c.clone());
        set.insert(none.clone());

        assert!(set.contains(&a));
        assert!(set.contains(&b));
        assert!(set.contains(&c));
        assert!(set.contains(&none));
        assert_eq!(set.len(), 3); // a/b, c, none
    }
}
