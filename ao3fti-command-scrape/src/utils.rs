use ao3fti_common::Uri;
use isahc::{
    config::{Configurable as _, RedirectPolicy},
    AsyncReadResponseExt as _, HttpClient, Request,
};
use rand::Rng;
use tracing::{Instrument, Span};

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
const USER_AGENT: &str = concat!(
    "Mozilla/5.0 (X11; Linux x86_64; rv:63.0) Servo/1.0 Firefox/63.0 aoooftis/",
    env!("CARGO_PKG_VERSION"),
    " (txuritan@protonmail.com)"
);
#[cfg(all(target_os = "linux", not(target_arch = "x86_64")))]
const USER_AGENT: &str = concat!(
    "Mozilla/5.0 (X11; Linux i686; rv:63.0) Servo/1.0 Firefox/63.0 aoooftis/",
    env!("CARGO_PKG_VERSION"),
    " (txuritan@protonmail.com)"
);

#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
const USER_AGENT: &str = concat!(
    "Mozilla/5.0 (Windows NT 6.1; Win64; x64; rv:63.0) Servo/1.0 Firefox/63.0 aoooftis/",
    env!("CARGO_PKG_VERSION"),
    " (txuritan@protonmail.com)"
);
#[cfg(all(target_os = "windows", not(target_arch = "x86_64")))]
const USER_AGENT: &str = concat!(
    "Mozilla/5.0 (Windows NT 6.1; rv:63.0) Servo/1.0 Firefox/63.0 aoooftis/",
    env!("CARGO_PKG_VERSION"),
    " (txuritan@protonmail.com)"
);

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
// Neither Linux nor Windows, so maybe OS X, and if not then OS X is an okay fallback.
const USER_AGENT: &str = concat!(
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.10; rv:63.0) Servo/1.0 Firefox/63.0 aoooftis/",
    env!("CARGO_PKG_VERSION"),
    " (txuritan@protonmail.com)"
);

#[cfg(target_os = "android")]
const USER_AGENT: &str = concat!(
    "Mozilla/5.0 (Android; Mobile; rv:63.0) Servo/1.0 Firefox/63.0 aoooftis/",
    env!("CARGO_PKG_VERSION"),
    " (txuritan@protonmail.com)"
);
#[cfg(target_os = "ios")]
const USER_AGENT: &str = concat!(
    "Mozilla/5.0 (iPhone; CPU iPhone OS 8_3 like Mac OS X; rv:63.0) Servo/1.0 Firefox/63.0 aoooftis/",
    env!("CARGO_PKG_VERSION"),
    " (txuritan@protonmail.com)"
);

#[tracing::instrument(err, skip(url), fields(url = %url.to_string()))]
pub(crate) async fn req(url: &Uri) -> Result<String, ao3fti_common::Report> {
    tracing::info!("fetching");

    let client = HttpClient::builder()
        .default_header("User-Agent", USER_AGENT)
        .default_header("Cookie", "view_adult=true")
        .build()?;

    let req = Request::builder()
        .redirect_policy(RedirectPolicy::Follow)
        .uri(url)
        .body(())?;

    let mut res = client.send_async(req).await?;

    let html = res.text().await?;

    Ok(html)
}

#[tracing::instrument(err)]
pub(crate) async fn sleep() -> Result<(), ao3fti_common::Report> {
    tokio::task::spawn_blocking({
        let span = Span::current();

        move || {
            let _ = span.enter();

            let length = rand::thread_rng().gen_range(3..8);

            tracing::info!("[util] Sleeping for {} seconds", length);

            std::thread::sleep(std::time::Duration::from_secs(length));
        }
    })
    .instrument(Span::current())
    .await
    .expect("Thread pool closed");

    Ok(())
}
