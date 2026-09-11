use percent_encoding::{NON_ALPHANUMERIC, percent_decode_str, utf8_percent_encode};

pub fn segment(value: &str) -> String {
    const ENCODE: &percent_encoding::AsciiSet = &NON_ALPHANUMERIC
        .remove(b'-')
        .remove(b'_')
        .remove(b'.')
        .remove(b'~')
        .remove(b':');
    utf8_percent_encode(value, ENCODE).to_string()
}

pub fn repo_route(repo: &str) -> String {
    format!("/{}", segment(repo))
}

pub fn path_route(repo: &str, op: &str, rev: &str, path: &str) -> String {
    let rev = rev.split('/').map(segment).collect::<Vec<_>>().join("/");
    let base = format!("{}/-/{op}/{rev}", repo_route(repo));
    if path.is_empty() {
        base
    } else {
        format!(
            "{base}/{}",
            path.split('/').map(segment).collect::<Vec<_>>().join("/")
        )
    }
}

pub fn query(route: &str, pairs: &[(&str, &str)]) -> String {
    format!(
        "{route}?{}",
        url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs(pairs.iter().copied())
            .finish()
    )
}

pub fn display_rev(rev: &str) -> String {
    rev.strip_prefix("tag/").unwrap_or(rev).replace(':', "/")
}

pub fn valid_path(path: &str) -> bool {
    path.is_empty()
        || path
            .split('/')
            .all(|part| !matches!(part, "" | "." | "..") && !part.contains('\0'))
}

pub fn decode(value: &str) -> Option<String> {
    percent_decode_str(value)
        .decode_utf8()
        .ok()
        .map(|s| s.into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_encode_segments_and_queries_once() {
        assert_eq!(
            path_route("my repo", "blob", "tag/release:1", "src/文 #?%.txt"),
            "/my%20repo/-/blob/tag/release:1/src/%E6%96%87%20%23%3F%25.txt"
        );
        assert_eq!(
            query("/repo/-/refs", &[("path", "a#?&%.txt"), ("rev", "tag/v1")]),
            "/repo/-/refs?path=a%23%3F%26%25.txt&rev=tag%2Fv1"
        );
        assert_eq!(display_rev("feature:login"), "feature/login");
        assert!(!valid_path("../file"));
        assert!(!valid_path("a//b"));
        assert!(valid_path("a/#?%.txt"));
    }
}
