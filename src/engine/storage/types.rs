use url::{Origin, Url};

/// Partitioning key (future-proof for state partitioning).
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum PartitionKey {
    None,
    TopLevel(Origin),
}

impl Default for PartitionKey {
    fn default() -> Self { PartitionKey::None }
}

impl PartitionKey {
    pub fn from_str(s: &str) -> Self {
        if s.is_empty() {
            PartitionKey::None
        } else {
            let url = Url::parse(s).expect("valid URL for PartitionKey");
            PartitionKey::TopLevel(url.origin())
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum PartitionPolicy { None, TopLevelOrigin }

pub fn compute_partition_key(u: &Url, p: PartitionPolicy) -> PartitionKey {
    match p {
        PartitionPolicy::None => PartitionKey::None,
        PartitionPolicy::TopLevelOrigin => PartitionKey::TopLevel(u.origin())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use url::Url;

    fn o(s: &str) -> Origin {
        let url = Url::parse(s).expect("valid URL");
        url.origin()
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
            PartitionKey::TopLevel(o) => {
                assert_eq!(o.ascii_serialization(), "https://sub.example.com:8443");
            }
            _ => panic!("expected TopLevel origin"),
        }
    }

    #[test]
    fn compute_toplevel_elides_default_port() {
        // For HTTPS default port (443), ascii_serialization omits the port.
        let u = Url::parse("https://example.com/anything").unwrap();
        let pk = compute_partition_key(&u, PartitionPolicy::TopLevelOrigin);
        assert_eq!(pk, PartitionKey::TopLevel(o("https://example.com")));
    }

    #[test]
    fn compute_toplevel_ipv6_with_port() {
        let u = Url::parse("http://[2001:db8::1]:8080/").unwrap();
        let pk = compute_partition_key(&u, PartitionPolicy::TopLevelOrigin);

        assert_eq!(pk, PartitionKey::from_str("http://[2001:db8::1]:8080"));
    }

    #[test]
    fn partitionkey_equality_and_hash_semantics() {
        use std::collections::HashSet;

        let a = PartitionKey::from_str("https://a.example");
        let b = PartitionKey::from_str("https://a.example");
        let c = PartitionKey::from_str("https://b.example");
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
