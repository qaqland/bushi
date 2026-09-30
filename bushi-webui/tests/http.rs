use axum::{
    Router,
    body::{Body, to_bytes},
    http::{HeaderMap, Request, StatusCode},
};
use bushi_webui::{
    config::AppConfig,
    data::{GitRepository, SqliteRepository},
    url::{path_route, query},
    web::{AppState, router},
};
use std::{fs, path::Path, process::Command};
use tower::ServiceExt;

fn git(repo: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .env("GIT_AUTHOR_NAME", "HTTP Test")
        .env("GIT_AUTHOR_EMAIL", "http@example.com")
        .env("GIT_COMMITTER_NAME", "HTTP Test")
        .env("GIT_COMMITTER_EMAIL", "http@example.com")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

async fn get(app: &Router, uri: &str) -> (StatusCode, HeaderMap, Vec<u8>) {
    let response = app
        .clone()
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = to_bytes(response.into_body(), 4 * 1024 * 1024)
        .await
        .unwrap();
    (status, headers, bytes.to_vec())
}

#[tokio::test]
async fn http_routes_redirects_and_downloads() {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("http repo");
    fs::create_dir_all(repo.join("docs")).unwrap();
    fs::create_dir_all(repo.join("gone")).unwrap();
    git(&repo, &["init", "-b", "main"]);
    let filename = "docs/a #?% 文.txt";
    let text = b"<script>not executable</script>\nsecond line\n";
    fs::write(repo.join(filename), text).unwrap();
    fs::write(repo.join("gone/file.txt"), "deleted content\n").unwrap();
    fs::write(
        repo.join("README.md"),
        "# Example\n\n<script>alert(1)</script>\n",
    )
    .unwrap();
    fs::write(repo.join("empty.txt"), "").unwrap();
    fs::write(repo.join("binary.dat"), b"hello\0world").unwrap();
    fs::write(repo.join("image.png"), b"\x89PNG\r\n\x1a\n\0").unwrap();
    fs::write(repo.join("unsafe.svg"), "<svg onload='alert(1)'></svg>").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "Initial files"]);
    let initial = git(&repo, &["rev-parse", "HEAD"]);
    git(&repo, &["tag", "release/1.0"]);
    git(&repo, &["branch", "feature/links"]);
    // Keep the two ref targets a whole second apart so their order is defined.
    std::thread::sleep(std::time::Duration::from_secs(1));
    git(&repo, &["rm", "gone/file.txt"]);
    fs::write(
        repo.join("long.txt"),
        (0..650).map(|i| format!("line {i}\n")).collect::<String>(),
    )
    .unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "Delete and add files"]);
    let head = git(&repo, &["rev-parse", "HEAD"]);

    let db = temp.path().join("index.db");
    let bin = std::env::var("BUSHI_INDEX_BIN")
        .unwrap_or_else(|_| "../bushi-index/build/bushi-index".to_string());
    for args in [
        vec!["-l"],
        vec!["-a", repo.to_str().unwrap()],
        vec!["http repo"],
    ] {
        let output = Command::new(&bin)
            .arg("-t")
            .arg(&db)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "index: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let app = router().with_state(AppState::new(
        SqliteRepository::new(db.to_str().unwrap()).unwrap(),
        GitRepository::new(),
        AppConfig::default(),
    ));

    // HTML routes are checked as HTTP interfaces, not DOM or class snapshots.
    for uri in [
        "/".to_string(),
        "/http%20repo".to_string(),
        "/http%20repo/-/refs".to_string(),
        "/http%20repo/-/refs?kind=branches&rev=feature%3Alinks".to_string(),
        "/http%20repo/-/refs?kind=tags&rev=tag%2Frelease%3A1.0".to_string(),
        "/http%20repo/-/tree/main".to_string(),
        "/http%20repo/-/tree/tag/release:1.0".to_string(),
        "/http%20repo/-/history/main".to_string(),
        "/http%20repo/-/history/main/gone/file.txt".to_string(),
        query("/http%20repo/-/history/main", &[("after", &head)]),
        path_route("http repo", "blob", "main", filename),
        "/http%20repo/-/blob/main/binary.dat".to_string(),
        "/http%20repo/-/blob/main/empty.txt".to_string(),
        path_route("http repo", "commit", &head, ""),
        path_route("http repo", "commit", &initial, ""),
        query(
            "/http%20repo/-/refs",
            &[("rev", "main"), ("path", filename), ("view", "blob")],
        ),
    ] {
        let (status, headers, body) = get(&app, &uri).await;
        assert_eq!(status, StatusCode::OK, "{uri}");
        assert_eq!(headers["content-type"], "text/html; charset=utf-8");
        assert!(!body.is_empty());
        // Stage timings ride on the standard Server-Timing header, not the body.
        let timing = headers["server-timing"].to_str().unwrap();
        assert!(timing.contains("render.body;dur="), "{uri}: {timing}");
        assert!(timing.contains("total;dur="), "{uri}: {timing}");
    }

    let (status, headers, css) = get(&app, "/static/bushi.css").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers["content-type"], "text/css; charset=utf-8");
    assert!(!css.is_empty());
    // Routes without any timed stage still report a total.
    let timing = headers["server-timing"].to_str().unwrap();
    assert!(timing.starts_with("total;dur="), "{timing}");

    for (uri, location) in [
        (
            "/http%20repo/-/tree".to_string(),
            "/http%20repo/-/tree/main".to_string(),
        ),
        (
            path_route("http repo", "blob", &head, "docs"),
            path_route("http repo", "tree", &head, "docs"),
        ),
        (
            query(
                "/http%20repo/-/refs",
                &[
                    ("rev", "main"),
                    ("path", filename),
                    ("view", "blob"),
                    ("to", "feature:links"),
                ],
            ),
            path_route("http repo", "blob", "feature:links", filename),
        ),
        (
            query(
                "/http%20repo/-/refs",
                &[
                    ("rev", "main"),
                    ("path", filename),
                    ("view", "blob"),
                    ("to", "tag/release:1.0"),
                ],
            ),
            path_route("http repo", "blob", "tag/release:1.0", filename),
        ),
        (
            query(
                "/http%20repo/-/refs",
                &[
                    ("rev", "feature:links"),
                    ("path", "gone/file.txt"),
                    ("view", "history"),
                    ("to", "main"),
                ],
            ),
            "/http%20repo/-/history/main/gone/file.txt".to_string(),
        ),
    ] {
        let (status, headers, _) = get(&app, &uri).await;
        assert_eq!(status, StatusCode::TEMPORARY_REDIRECT, "{uri}");
        assert_eq!(headers["location"], location);
        assert_eq!(get(&app, &location).await.0, StatusCode::OK);
    }

    for (path, content_type, expected) in [
        (filename, "text/plain; charset=utf-8", text.as_slice()),
        ("image.png", "image/png", b"\x89PNG\r\n\x1a\n\0".as_slice()),
        (
            "unsafe.svg",
            "text/plain; charset=utf-8",
            b"<svg onload='alert(1)'></svg>".as_slice(),
        ),
        (
            "binary.dat",
            "application/octet-stream",
            b"hello\0world".as_slice(),
        ),
        ("empty.txt", "text/plain; charset=utf-8", b"".as_slice()),
    ] {
        let uri = path_route("http repo", "raw", "main", path);
        let (status, headers, body) = get(&app, &uri).await;
        assert_eq!(status, StatusCode::OK, "{uri}");
        assert_eq!(headers["content-type"], content_type);
        assert_eq!(headers["x-content-type-options"], "nosniff");
        assert_eq!(
            headers["content-security-policy"],
            "default-src 'none'; sandbox"
        );
        assert_eq!(body, expected);
        if path == "binary.dat" {
            assert!(
                headers["content-disposition"]
                    .to_str()
                    .unwrap()
                    .starts_with("attachment")
            );
        }
    }
    let download = query(
        &path_route("http repo", "raw", "main", filename),
        &[("download", "1")],
    );
    let (status, headers, body) = get(&app, &download).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, text);
    assert_eq!(
        headers["content-disposition"],
        "attachment; filename*=UTF-8''a%20%23%3F%25%20%E6%96%87.txt"
    );

    let patch_url = path_route("http repo", "commit", &format!("{head}.patch"), "");
    let (status, headers, patch) = get(&app, &patch_url).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers["content-type"], "text/x-patch; charset=utf-8");
    assert_eq!(headers["x-content-type-options"], "nosniff");
    assert_eq!(
        headers["content-security-policy"],
        "default-src 'none'; sandbox"
    );
    assert!(
        headers["content-disposition"]
            .to_str()
            .unwrap()
            .starts_with("attachment")
    );
    assert!(String::from_utf8_lossy(&patch).contains("+line 649"));

    for uri in [
        "/no-such-repo".to_string(),
        "/http%20repo/-/unknown".to_string(),
        "/http%20repo/-/blob/main/missing.txt".to_string(),
        query(
            "/http%20repo/-/refs",
            &[
                ("rev", "feature:links"),
                ("path", "gone/file.txt"),
                ("view", "blob"),
                ("to", "main"),
            ],
        ),
    ] {
        assert_eq!(get(&app, &uri).await.0, StatusCode::NOT_FOUND, "{uri}");
    }
    for uri in [
        "/http%20repo/-/refs?kind=other".to_string(),
        "/http%20repo/-/refs?view=https://evil.example".to_string(),
        "/http%20repo/-/refs?to=main".to_string(),
        "/http%20repo/-/raw/main/empty.txt?download=invalid".to_string(),
        query(
            "/http%20repo/-/refs",
            &[("path", "../outside"), ("view", "blob")],
        ),
    ] {
        assert_eq!(get(&app, &uri).await.0, StatusCode::BAD_REQUEST, "{uri}");
    }
}
