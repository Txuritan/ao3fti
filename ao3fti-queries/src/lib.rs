use std::collections::HashMap;

use ao3fti_common::{
    models::{Entity, Rating, Story},
    Conf,
};
use dataloader::{cached::Loader, BatchFn};

use sqlx::{migrate::Migrator, sqlite::SqlitePoolOptions, Connection, Sqlite, Transaction};

pub use sqlx::SqlitePool as Pool;

pub type PgTransaction<'l> = Transaction<'l, Sqlite>;

#[tracing::instrument(skip(conf), err)]
pub async fn init_database_connection(conf: &Conf) -> Result<Pool, ao3fti_common::Report> {
    static MIGRATOR: Migrator = sqlx::migrate!();

    let pool = SqlitePoolOptions::new().connect(&conf.database).await?;

    MIGRATOR.run(&pool).await?;

    Ok(pool)
}

#[tracing::instrument(skip(trans), err)]
pub async fn check_story_if_exists(
    trans: &mut Transaction<'_, Sqlite>,
    story_id: usize,
) -> Result<bool, ao3fti_common::Report> {
    let story_id = story_id as i64;
    let existing = sqlx::query!("SELECT id FROM stories WHERE id = ?", story_id)
        .fetch_optional(&mut *trans)
        .await?;

    Ok(existing.is_some())
}

pub async fn get_story_count(pool: Pool) -> Result<i64, ao3fti_common::Report> {
    struct Count {
        estimate: i32,
    }

    let count = sqlx::query_as!(Count, "SELECT COUNT(1) as estimate FROM stories;")
        .fetch_one(&pool)
        .await?;

    Ok(count.estimate as i64)
}

macro_rules! get_or_create {
    ($fn_name:ident, $select:expr, $insert:expr) => {
        #[tracing::instrument(skip(trans), err)]
        pub async fn $fn_name(
            trans: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
            name: &str,
        ) -> Result<i32, ao3fti_common::Report> {
            let find = sqlx::query!($select, name)
                .fetch_optional(&mut *trans)
                .await?;

            if let Some(record) = find {
                tracing::debug!("found existing entity");

                Ok(record.id as i32)
            } else {
                tracing::debug!("creating entity");

                sqlx::query!($insert, name).execute(&mut *trans).await?;

                let record = sqlx::query!($select, name).fetch_one(&mut *trans).await?;

                Ok(record.id as i32)
            }
        }
    };
}

get_or_create!(
    get_or_create_author,
    "SELECT id FROM authors WHERE name = ?",
    "INSERT INTO authors(name) VALUES (?)"
);

get_or_create!(
    get_or_create_origin,
    "SELECT id FROM origins WHERE name = ?",
    "INSERT INTO origins(name) VALUES (?)"
);

get_or_create!(
    get_or_create_warning,
    "SELECT id FROM warnings WHERE name = ?",
    "INSERT INTO warnings(name) VALUES (?)"
);

get_or_create!(
    get_or_create_pairing,
    "SELECT id FROM pairings WHERE name = ?",
    "INSERT INTO pairings(name) VALUES (?)"
);

get_or_create!(
    get_or_create_character,
    "SELECT id FROM characters WHERE name = ?",
    "INSERT INTO characters(name) VALUES (?)"
);

get_or_create!(
    get_or_create_general,
    "SELECT id FROM generals WHERE name = ?",
    "INSERT INTO generals(name) VALUES (?)"
);

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct Info {
    pub name: String,
    pub authors: Vec<String>,
    pub summary: String,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct Meta {
    pub rating: Rating,
    pub categories: Vec<String>,
    pub origins: Vec<String>,
    pub warnings: Vec<String>,
    pub pairings: Vec<String>,
    pub characters: Vec<String>,
    pub generals: Vec<String>,
}

macro_rules! for_tag {
    ($pool:ident, $story_id:ident, [ $( $member:expr => ($get_or_create:ident, $insert:expr); )* ]) => {{
        $(
            for entity in &$member {
                let id = $get_or_create(&mut *$pool, entity).await?;

                tracing::debug!(story_id = %$story_id, entity_id = %id, "linking entity to story");

                let story_id = $story_id as i32;
                sqlx::query!($insert, story_id, id)
                    .execute(&mut *$pool)
                    .await?;
            }
        )*
    }};
}

#[tracing::instrument(skip(trans, info, meta), err)]
pub async fn insert_story(
    trans: &mut Transaction<'_, Sqlite>,
    story_id: usize,
    info: Info,
    meta: Meta,
) -> Result<bool, ao3fti_common::Report> {
    let story_id = story_id as i32;
    let rating = serde_plain::to_string(&meta.rating).unwrap();
    sqlx::query!(
        "INSERT INTO stories(id, name, summary, rating) VALUES (?, ?, ?, ?)",
        story_id,
        info.name,
        info.summary,
        rating,
    )
    .execute(&mut *trans)
    .await?;

    #[rustfmt::skip]
    for_tag!(trans, story_id, [
        info.authors => (get_or_create_author, "INSERT INTO story_authors(story_id, author_id) VALUES (?, ?)");
    ]);

    #[rustfmt::skip]
    for_tag!(trans, story_id, [
        meta.origins => (get_or_create_origin, "INSERT INTO story_origins(story_id, origin_id) VALUES (?, ?)");
        meta.warnings => (get_or_create_warning, "INSERT INTO story_warnings(story_id, warning_id) VALUES (?, ?)");
        meta.pairings => (get_or_create_pairing, "INSERT INTO story_pairings(story_id, pairing_id) VALUES (?, ?)");
        meta.characters => (get_or_create_character, "INSERT INTO story_characters(story_id, character_id) VALUES (?, ?)");
        meta.generals => (get_or_create_general, "INSERT INTO story_generals(story_id, general_id) VALUES (?, ?)");
    ]);

    Ok(false)
}

macro_rules! loader {
    ($name:ident, $select:expr) => {
        struct $name {
            pool: Pool,
        }

        #[async_trait::async_trait]
        impl BatchFn<i32, Entity> for $name {
            #[tracing::instrument(skip(self))]
            async fn load(&mut self, keys: &[i32]) -> HashMap<i32, Entity> {
                let mut entries = HashMap::new();
                for key in keys {
                    match sqlx::query!($select, key).fetch_one(&self.pool).await {
                        Ok(entity) => {
                            entries.insert(*key, Entity { name: entity.name });
                        },
                        Err(err) => {
                            tracing::error!(err = ?err, "unable to load entities");

                            return HashMap::new();
                        }
                    }
                }
                entries
            }
        }
    }
}

loader!(AuthorLoader, "SELECT name FROM authors WHERE id = ?");
loader!(OriginLoader, "SELECT name FROM origins WHERE id = ?");
loader!(WarningLoader, "SELECT name FROM warnings WHERE id = ?");
loader!(PairingLoader, "SELECT name FROM pairings WHERE id = ?");
loader!(CharacterLoader, "SELECT name FROM characters WHERE id = ?");
loader!(GeneralLoader, "SELECT name FROM generals WHERE id = ?");

pub struct Loaders {
    author: Loader<i32, Entity, AuthorLoader>,
    origin: Loader<i32, Entity, OriginLoader>,
    warning: Loader<i32, Entity, WarningLoader>,
    pairing: Loader<i32, Entity, PairingLoader>,
    character: Loader<i32, Entity, CharacterLoader>,
    general: Loader<i32, Entity, GeneralLoader>,
}

impl Loaders {
    pub fn new(pool: Pool) -> Self {
        Self {
            author: Loader::new(AuthorLoader { pool: pool.clone() }),
            origin: Loader::new(OriginLoader { pool: pool.clone() }),
            warning: Loader::new(WarningLoader { pool: pool.clone() }),
            pairing: Loader::new(PairingLoader { pool: pool.clone() }),
            character: Loader::new(CharacterLoader { pool: pool.clone() }),
            general: Loader::new(GeneralLoader { pool }),
        }
    }
}

macro_rules! load {
    ($pool:ident, $loaders:ident . $loader:ident, $query:expr, $id:expr) => {{
        let id_records = sqlx::query!($query, $id).fetch_all(&$pool).await?;

        let ids = id_records
            .into_iter()
            .map(|r| r.id as i32)
            .collect::<Vec<i32>>();

        let mut entities: HashMap<i32, Entity> = $loaders.$loader.load_many(ids.clone()).await;

        let mut collected = Vec::with_capacity(ids.len());
        for id in ids {
            collected.push(entities.remove(&id).unwrap());
        }
        collected
    }};
}

#[rustfmt::skip]
#[tracing::instrument(skip(pool, loaders), err)]
pub async fn get_story(pool: Pool, loaders: &Loaders, story_id: u64) -> Result<Story, ao3fti_common::Report> {
    let id = story_id as i32;

    let story = sqlx::query!("SELECT name, summary, rating FROM stories WHERE id = ?", id).fetch_one(&pool).await?;

    let authors = load!(pool, loaders.author, "SELECT author_id as id FROM story_authors WHERE story_id = ? ORDER BY created DESC", id);
    let origins = load!(pool, loaders.origin, "SELECT origin_id as id FROM story_origins WHERE story_id = ? ORDER BY created DESC", id);
    let warnings = load!(pool, loaders.warning, "SELECT warning_id as id FROM story_warnings WHERE story_id = ? ORDER BY created DESC", id);
    let pairings = load!(pool, loaders.pairing, "SELECT pairing_id as id FROM story_pairings WHERE story_id = ? ORDER BY created DESC", id);
    let characters = load!(pool, loaders.character, "SELECT character_id as id FROM story_characters WHERE story_id = ? ORDER BY created DESC", id);
    let generals = load!(pool, loaders.general, "SELECT general_id as id FROM story_generals WHERE story_id = ? ORDER BY created DESC", id);

    Ok(Story {
        id: id as usize,
        name: story.name,
        summary: story.summary,
        authors,
        origins,
        warnings,
        pairings,
        characters,
        generals,
    })
}

#[tracing::instrument(skip(pool, uris), err)]
pub async fn queue_insert(pool: Pool, uris: &[String]) -> Result<(), ao3fti_common::Report> {
    let mut conn = pool.acquire().await?;
    let mut trans = conn.begin().await?;

    for uri in uris {
        sqlx::query!("INSERT INTO page_queue(uri) VALUES (?)", uri)
            .execute(&mut trans)
            .await?;
    }

    trans.commit().await?;

    Ok(())
}

#[tracing::instrument(skip(pool), err)]
pub async fn queue_next(pool: Pool) -> Result<Option<String>, ao3fti_common::Report> {
    let next =
        sqlx::query!("SELECT uri FROM page_queue WHERE completed = FALSE ORDER BY created DESC")
            .fetch_optional(&pool)
            .await?;

    Ok(next.map(|r| r.uri))
}

// https://github.com/ayrat555/fang
pub async fn queue_task(pool: Pool) -> Result<(), ao3fti_common::Report> {
    use tracing::Instrument as _;

    async fn inner(pool: Pool) -> Result<(), ao3fti_common::Report> {
        loop {
            if let Some(_uri) = queue_next(pool.clone()).await? {}

            ao3fti_common::utils::sleep().await?;
        }
    }

    inner(pool).in_current_span().await
}
