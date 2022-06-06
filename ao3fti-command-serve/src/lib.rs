use std::{sync::Arc, time::Duration};

use ao3fti_common::models::Story;
use ao3fti_indexer::{
    Hit, IndexServer, NamedFieldDocument, SearchQuery as ApiSearchQuery, Serp, Value,
};
use ao3fti_queries::{Loaders, PgPool};
use askama::Template;
use axum::{
    error_handling::HandleErrorLayer,
    extract::{Extension, Query},
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::get,
    BoxError, Json, Router, Server,
};
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;

pub async fn run() -> Result<(), ao3fti_common::Report> {
    let pool = ao3fti_queries::init_database_connection().await?;
    let index_server = IndexServer::new()?;

    let app: _ = Router::new()
        .route("/", get(index))
        .route("/search", get(search_html))
        .route("/api", get(search_api))
        .layer(
            ServiceBuilder::new()
                .layer(HandleErrorLayer::new(|error: BoxError| async move {
                    if error.is::<tower::timeout::error::Elapsed>() {
                        (StatusCode::REQUEST_TIMEOUT, String::new())
                    } else {
                        (StatusCode::INTERNAL_SERVER_ERROR, String::new())
                    }
                }))
                .load_shed()
                .concurrency_limit(1024)
                .timeout(Duration::from_secs(10))
                .layer(Extension(pool))
                .layer(Extension(index_server))
                .layer(TraceLayer::new_for_http())
                .into_inner(),
        );

    tracing::info!("starting on `0.0.0.0:8080`");

    Server::bind(&"0.0.0.0:8080".parse()?)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}

static STYLE: &str = include_str!("../../assets/dest.min.css");

#[derive(askama::Template)]
#[template(path = "index.html")]
struct IndexPage {
    css: &'static str,
    stories: i64,
}

async fn index(Extension(pool): Extension<PgPool>) -> Result<impl IntoResponse, Error> {
    let count = ao3fti_queries::get_story_count(pool).await?;

    Ok(Html(
        IndexPage {
            css: STYLE,
            stories: count,
        }
        .render()
        .map_err(Error::from_any)?,
    ))
}

async fn search_api(
    Extension(index): Extension<Arc<IndexServer>>,
    Query(search): Query<ApiSearchQuery>,
) -> Result<impl IntoResponse, Error> {
    let serp = tokio::task::spawn_blocking(move || -> Result<Serp, ao3fti_common::Report> {
        ao3fti_indexer::serp(index, search)
    })
    .await;

    let serp = match serp {
        Ok(serp) => serp?,
        Err(err) => return Err(Error::from_any(err)),
    };

    Ok(Json(serp))
}

#[derive(Debug, serde::Deserialize)]
pub struct SearchQuery {
    query: String,
    page: usize,
}

#[derive(Debug, serde::Serialize)]
pub struct SearchQueryPart<'q> {
    query: &'q str,
}

#[derive(askama::Template)]
#[template(path = "search.html")]
struct Search {
    css: &'static str,
    query: String,
    stories: Vec<Story>,
    pagination: Pagination,
}

async fn search_html(
    Extension(pool): Extension<PgPool>,
    Extension(index): Extension<Arc<IndexServer>>,
    Query(search): Query<SearchQuery>,
) -> Result<impl IntoResponse, Error> {
    const SEARCH_LIMIT: usize = 20;

    let api_search = ApiSearchQuery {
        query: search.query.clone(),
        offset: 20 * (search.page - 1),
        limit: SEARCH_LIMIT,
    };

    let serp = tokio::task::spawn_blocking(move || -> Result<Serp, ao3fti_common::Report> {
        ao3fti_indexer::serp(index, api_search)
    })
    .await;

    let Serp { hits, num_hits, .. } = match serp {
        Ok(serp) => serp?,
        Err(err) => return Err(Error::from_any(err)),
    };

    let loaders = Loaders::new(pool.clone());
    let mut stories = Vec::with_capacity(hits.len());

    for Hit {
        doc: NamedFieldDocument(map),
        ..
    } in hits
    {
        let story_id_value = map.get("story_id").and_then(|l| l.iter().next());
        if let Some(Value::U64(story_id)) = story_id_value {
            stories.push(ao3fti_queries::get_story(pool.clone(), &loaders, *story_id).await?);
        }
    }

    let url_fragment = serde_urlencoded::to_string(&SearchQueryPart {
        query: &search.query,
    })
    .map_err(Error::from_any)?;

    Ok(Html(
        Search {
            css: STYLE,
            query: search.query,
            stories,
            pagination: Pagination::new(
                url_fragment,
                search.page,
                (num_hits + SEARCH_LIMIT - 1) / SEARCH_LIMIT,
            ),
        }
        .render()
        .map_err(Error::from_any)?,
    ))
}

pub struct Pagination {
    prev: Link,
    parts: Vec<Link>,
    next: Link,
}

impl Pagination {
    pub fn new(fragment: String, page: usize, pages: usize) -> Self {
        let (prev, parts, next) = Self::paginate(&fragment, pages, page);

        Self { prev, parts, next }
    }

    fn paginate(fragment: &str, pages: usize, page: usize) -> (Link, Vec<Link>, Link) {
        let mut buff = Vec::with_capacity(11);

        let prev = Link {
            state: if page == 1 {
                LinkState::Disabled
            } else {
                LinkState::Normal
            },
            href: format!("?{}&page={}", fragment, page),
            text: "previous".into(),
        };

        for i in 1..=pages {
            if i == 1 {
                buff.push(Pager::Num(i == page, i));

                continue;
            }

            if i == pages {
                buff.push(Pager::Num(i == page, i));

                continue;
            }

            if (page.checked_sub(1).unwrap_or(page)..=page.checked_add(1).unwrap_or(page))
                .contains(&i)
            {
                buff.push(Pager::Num(i == page, i));
            } else if let Some(l) = buff.last_mut() {
                if *l == Pager::Ellipse {
                    continue;
                } else {
                    buff.push(Pager::Ellipse);
                }
            }
        }

        let next = Link {
            state: if page == pages {
                LinkState::Disabled
            } else {
                LinkState::Normal
            },
            href: format!("?{}&page={}", fragment, page),
            text: "next".into(),
        };

        let buff = buff
            .into_iter()
            .map(|pager| match pager {
                Pager::Num(active, page) => Link {
                    state: if active {
                        LinkState::Active
                    } else {
                        LinkState::Normal
                    },
                    href: format!("?{}&page={}", fragment, page),
                    text: page.into_readable().to_string(),
                },
                Pager::Ellipse => Link {
                    state: LinkState::Normal,
                    href: "#".into(),
                    text: "..".into(),
                },
            })
            .collect::<Vec<_>>();

        (prev, buff, next)
    }
}

#[derive(Debug, PartialEq)]
enum Pager {
    Num(bool, usize),
    Ellipse,
}

struct Link {
    state: LinkState,
    href: String,
    text: String,
}

#[derive(Debug, PartialEq)]
enum LinkState {
    Normal,
    Active,
    Disabled,
}

#[derive(Debug)]
pub struct Error(ao3fti_common::Report);

impl Error {
    pub fn from_any<A>(err: A) -> Self
    where
        A: Into<ao3fti_common::Report>,
    {
        Self(err.into())
    }
}

impl From<ao3fti_common::Report> for Error {
    fn from(err: ao3fti_common::Report) -> Self {
        Self(err)
    }
}

impl axum::response::IntoResponse for Error {
    fn into_response(self) -> axum_core::response::Response {
        #[derive(serde::Serialize)]
        struct Res {
            error: ResErr,
        }

        #[derive(serde::Serialize)]
        struct ResErr {
            code: u16,
            status: &'static str,
        }

        let err = self.0;

        tracing::error!(error = ?err, "error handling request");

        let (status, message) = (StatusCode::INTERNAL_SERVER_ERROR, "internal server error");

        let body = Res {
            error: ResErr {
                code: status.as_u16(),
                status: message,
            },
        };

        (status, Json(body)).into_response()
    }
}

pub struct Readable<N>
where
    N: std::fmt::Display,
{
    inner: N,
}

impl<N> std::fmt::Display for Readable<N>
where
    N: std::fmt::Display,
{
    #[allow(clippy::needless_collect)]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let values: Vec<(Option<char>, char)> = self
            .inner
            .to_string()
            .chars()
            .rev()
            .enumerate()
            .map(|(i, c)| {
                (
                    if i % 3 == 0 && i != 0 {
                        Some(',')
                    } else {
                        None
                    },
                    c,
                )
            })
            .collect();

        for (s, c) in values.into_iter().rev() {
            write!(f, "{}", c)?;

            if let Some(c) = s {
                write!(f, "{}", c)?;
            }
        }

        Ok(())
    }
}

pub trait IntoReadable: std::fmt::Display + Sized {
    fn into_readable(self) -> Readable<Self> {
        Readable { inner: self }
    }
}

impl IntoReadable for usize {}
impl IntoReadable for isize {}

impl IntoReadable for u32 {}
impl IntoReadable for u64 {}

impl IntoReadable for i32 {}
impl IntoReadable for i64 {}
