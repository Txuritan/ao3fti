mod query;
mod utils;

use ao3fti_common::{
    channel::{self, Sender},
    err,
    models::Rating,
    Conf, Context as _, Report, Uri,
};
use ao3fti_indexer::ChapterLine;
use ao3fti_queries::{Info, Meta, PgPool, PgTransaction};
use futures::future::TryFutureExt as _;
use tracing::{Instrument as _, Span};

#[tracing::instrument(skip(conf, url), err)]
pub async fn run(conf: &Conf, url: &str) -> Result<(), ao3fti_common::Report> {
    let pool = ao3fti_queries::init_database_connection(conf).await?;

    let (line_sender, line_receiver) = channel::bounded(10_000);

    let background_worker = tokio::task::spawn_blocking({
        let span = Span::current();

        move || span.in_scope(|| ao3fti_indexer::index(line_receiver))
    })
    .map_err(Report::from);

    #[tracing::instrument(skip(pool, url, line_sender), err)]
    async fn inner(
        pool: PgPool,
        url: &str,
        line_sender: Sender<ChapterLine>,
    ) -> Result<(), ao3fti_common::Report> {
        let base_url = Uri::try_from(url)
            .with_context(|| format!("with url, at line {}: `{}`", line!(), url))?;

        tracing::info!("starting scrape of search page");

        let mut url = Some(base_url.clone());
        let mut page_index = 1;

        while let Some(url_ref) = &url {
            let span =
                tracing::debug_span!("search page loop", page_index = page_index).or_current();

            tracing::info!(url = %url_ref.to_string(), "scraping search page");

            url = scrape_page(pool.clone(), &line_sender, &base_url, url_ref)
                .instrument(span.clone())
                .await?
                .map(|url| rebuild_url(&base_url, &url))
                .transpose()?;

            page_index += 1;

            utils::sleep().instrument(span.clone()).await?;
        }

        Ok(())
    }

    // TODO(txuritan): make this be only one Result
    tracing::debug!("starting background indexer and scraper");
    let (res, _) = tokio::try_join!(background_worker, inner(pool, url, line_sender))?;
    let _ = res?;

    Ok(())
}

#[tracing::instrument(skip(pool, line_sender, base_url, page_url), err)]
async fn scrape_page(
    pool: PgPool,
    line_sender: &channel::Sender<ChapterLine>,
    base_url: &Uri,
    page_url: &Uri,
) -> Result<Option<Uri>, ao3fti_common::Report> {
    static LIST_SELECTOR: &str = "html > body > #outer > #inner > #main > ol.work.index.group > li";
    static INFO_SELECTOR: &str = ".header.module > h4.heading > a";
    static NEXT_SELECTOR: &str =
        "html > body > #outer > #inner > #main > ol.pagination.actions > li > a[rel=next]";
    static RESTRICTED_SELECTOR: &str = "div.header.module > h4.heading > img[alt=(Restricted)])";

    let html = utils::req(page_url).await?;

    let doc = query::Document::try_from(html.as_str())?;

    for (story_index, story_element) in doc.select(LIST_SELECTOR).into_iter().enumerate() {
        tracing::info!(story_index = story_index, "working on story with index of");
        let restricted = story_element.select(RESTRICTED_SELECTOR);
        if !restricted.is_empty() {
            continue;
        }

        utils::sleep().instrument(Span::current()).await?;

        let story_link_element = story_element
            .select(INFO_SELECTOR)
            .into_iter()
            .next()
            .ok_or_else(|| err!("unable to find a story url on page `{}`", page_url))?;

        let story_link = story_link_element
            .attr("href")
            .ok_or_else(|| err!("unable to get href attribute on page `{}`", page_url))?;
        let story_link = format!("{}?view_adult=true", story_link);

        let story_url = Uri::try_from(story_link.as_str())
            .with_context(|| format!("with url, at line {}: `{}`", line!(), story_link))?;
        let story_url = rebuild_url(base_url, &story_url)?;

        let mut trans = pool.begin().await?;

        scrape_story(&mut trans, line_sender, base_url, &story_url).await?;

        trans.commit().await?;
    }

    Ok(match doc.select(NEXT_SELECTOR).into_iter().last() {
        Some(element) => {
            if element.text().unwrap() == "Next →" {
                element
                    .attr("href")
                    .map(|url| {
                        Uri::try_from(url.as_str())
                            .with_context(|| format!("with url, at line {}: `{}`", line!(), url))
                    })
                    .transpose()
                    .with_context(|| format!("invalid next url for page `{}`", page_url))?
            } else {
                None
            }
        }
        None => None,
    })
}

#[tracing::instrument(skip(trans, line_sender, base_url, story_url), fields(story_url = %story_url.to_string()), err)]
async fn scrape_story(
    trans: &mut PgTransaction<'_>,
    line_sender: &channel::Sender<ChapterLine>,
    base_url: &Uri,
    story_url: &Uri,
) -> Result<(), ao3fti_common::Report> {
    static CHAPTERS_SELECTOR: &str = "#chapters > .userstuff";

    let story_id = story_url
        .path()
        .split('/')
        .filter(|s| !s.is_empty())
        .nth(1)
        .ok_or_else(|| err!("No story ID found in URL"))?;
    let story_id = story_id.parse::<usize>()?;

    if ao3fti_queries::check_story_if_exists(trans, story_id).await? {
        tracing::warn!("story already exists");

        return Ok(());
    }

    tracing::info!(url = %story_url.to_string(), "scraping story");

    let download_url = get_download_url(story_url).await?;
    let download_url = Uri::try_from(download_url.as_str())
        .with_context(|| format!("with url, at line {}: `{}`", line!(), download_url))?;
    let download_url = rebuild_url(base_url, &download_url)?;

    utils::sleep().instrument(Span::current()).await?;

    let download_html = utils::req(&download_url).await?;

    let download_doc = query::Document::try_from(download_html.as_str())?;

    let info = get_story_info(story_url, &download_doc)?;

    let meta = get_story_meta(&download_doc);

    if ao3fti_queries::insert_story(trans, story_id, info, meta).await? {
        return Ok(());
    }

    for (chapter_id, chapter) in download_doc
        .select(CHAPTERS_SELECTOR)
        .into_iter()
        .enumerate()
    {
        tracing::debug!(story_id = %story_id, chapter_number = %chapter_id, "indexing chapter");

        let chapter_content = chapter.text().unwrap();

        line_sender
            .send(ChapterLine {
                story_id,
                chapter_id,
                chapter_content,
            })
            .context("error sending chapter to indexer")?;
    }

    Ok(())
}

#[tracing::instrument(skip(story_url), err)]
async fn get_download_url(story_url: &Uri) -> Result<String, ao3fti_common::Report> {
    static STORY_MULTI_DOWNLOAD_BUTTON: &str =
        "html > body > #outer > #inner > #main > .work > .navigation.actions > .download > ul > li > a";
    static STORY_SINGLE_DOWNLOAD_BUTTON: &str =
        "html > body > #outer > #inner > #main > .work.navigation.actions > .download > ul > li > a";

    let story_html = utils::req(story_url).await?;

    let doc = query::Document::try_from(story_html.as_str())?;

    let download_elements = {
        let temp = doc.select(STORY_MULTI_DOWNLOAD_BUTTON);

        if temp.is_empty() {
            doc.select(STORY_SINGLE_DOWNLOAD_BUTTON)
        } else {
            temp
        }
    };

    let download_element = {
        let this = download_elements.into_iter().last();

        match this {
            Some(v) => Ok(v),
            None => {
                tokio::fs::write("./output.html", &story_html)
                    .await
                    .with_context(|| {
                        format!(
                            "unable to write out for error debugging url: `{}`",
                            story_url
                        )
                    })?;

                Err(err!(
                    "Unable to select the download link for `{}`",
                    story_url
                ))
            }
        }
    }?;

    let href = download_element
        .attr("href")
        .ok_or_else(|| err!("download link does not have a href for `{}`", story_url))?;

    Ok(href)
}

#[tracing::instrument(skip(story_url, doc), err)]
fn get_story_info(story_url: &Uri, doc: &query::Document) -> Result<Info, ao3fti_common::Report> {
    static STORY_NAME: &str = "html > body > #preface > .meta > h1";
    static STORY_AUTHOR: &str = "html > body > #preface > .meta > .byline > a[rel=\"author\"]";
    static STORY_SUMMARY: &str = "html > body > #preface > .meta > blockquote";

    let name = doc
        .select(STORY_NAME)
        .into_iter()
        .next()
        .and_then(|element| element.text())
        .map(|mut name| {
            string_trim(&mut name);

            name
        })
        .ok_or_else(|| err!("unable to scrape name"))
        .with_context(|| format!("with url, at line {}: `{}`", line!(), story_url))?;

    let authors = doc
        .select(STORY_AUTHOR)
        .into_iter()
        .map(|element| {
            element.text().map(|mut name| {
                string_trim(&mut name);

                name
            })
        })
        .collect::<Option<Vec<String>>>()
        .ok_or_else(|| err!("unable to scrape authors"))
        .with_context(|| format!("with url, at line {}: `{}`", line!(), story_url))?;

    let summary = doc
        .select(STORY_SUMMARY)
        .into_iter()
        .next()
        .and_then(|element| element.inner_html())
        .unwrap_or_else(|| String::from("<p></p>"));

    Ok(Info {
        name,
        authors,
        summary,
    })
}

fn get_story_meta(doc: &query::Document) -> Meta {
    static META_TAGS_DT: &str = "html > body > #preface > .meta > .tags > dt";
    static META_TAGS_DF: &str = "html > body > #preface > .meta > .tags > dd";

    let mut rating = Rating::Unknown;

    let mut categories = Vec::new();

    let mut origins = Vec::new();

    let mut warnings = Vec::new();
    let mut pairings = Vec::new();
    let mut characters = Vec::new();
    let mut generals = Vec::new();

    let detail_names = doc.select(META_TAGS_DT);
    let detail_definitions = doc.select(META_TAGS_DF);

    let nodes = detail_names.into_iter().zip(detail_definitions.into_iter());
    for (detail_names, detail_definition) in nodes {
        let text = match detail_names.text().map(|mut text| {
            string_trim(&mut text);

            text
        }) {
            Some(text) => text,
            None => continue,
        };

        let list = match text.as_str() {
            "Rating:" => {
                let text = detail_definition.children().get(0).and_then(|node| {
                    node.text().map(|mut text| {
                        string_trim(&mut text);

                        text
                    })
                });

                match text.as_deref() {
                    Some("Explicit") => rating = Rating::Explicit,
                    Some("Mature") => rating = Rating::Mature,
                    Some("Teen And Up Audiences") => rating = Rating::Teen,
                    Some("General Audiences") => rating = Rating::General,
                    Some("Not Rated") => rating = Rating::NotRated,
                    _ => (),
                }

                None
            }
            "Archive Warning:" => Some(&mut warnings),
            "Category:" => Some(&mut categories),
            "Fandom:" => Some(&mut origins),
            "Relationship:" => Some(&mut pairings),
            "Character:" => Some(&mut characters),
            "Additional Tags:" => Some(&mut generals),
            _ => None,
        };

        if let Some(list) = list {
            for child in &detail_definition.children() {
                if let Some(text) = child.text() {
                    list.push(text.to_string());
                }
            }
        }
    }

    Meta {
        rating,
        categories,
        origins,
        warnings,
        pairings,
        characters,
        generals,
    }
}

fn rebuild_url(base_url: &Uri, new_url: &Uri) -> Result<Uri, ao3fti_common::Report> {
    let uri = Uri::builder()
        .scheme(
            base_url
                .scheme()
                .cloned()
                .ok_or_else(|| err!("base url is missing a scheme"))?,
        )
        .authority(
            base_url
                .authority()
                .cloned()
                .ok_or_else(|| err!("base url is missing a authority"))?,
        )
        .path_and_query(
            new_url
                .path_and_query()
                .cloned()
                .ok_or_else(|| err!("new url is missing path, query, and fragment"))?,
        )
        .build()
        .context("unable to build new url")?;

    Ok(uri)
}

fn string_trim(text: &mut String) {
    string_trim_start(text);
    string_trim_end(text);
}

fn string_trim_start(text: &mut String) {
    while text.starts_with(char::is_whitespace) {
        text.drain(..1);
    }
}

fn string_trim_end(text: &mut String) {
    while text.ends_with(char::is_whitespace) {
        text.truncate(text.len().saturating_sub(1));
    }
}
