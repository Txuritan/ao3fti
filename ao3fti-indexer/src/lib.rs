use std::sync::Arc;

use ao3fti_common::{
    channel::{self, Receiver},
    timer::TimerTree,
};
use tantivy::{
    collector::{Count, TopDocs},
    query::QueryParser,
    schema::{Field, FieldType, Schema},
    Document, Index, IndexReader, IndexWriter, Score,
};

pub use tantivy::schema::{NamedFieldDocument, Value};

#[derive(serde::Deserialize, serde::Serialize)]
pub struct ChapterLine {
    pub story_id: usize,
    pub chapter_id: usize,
    pub chapter_content: String,
}

#[tracing::instrument(skip(line_receiver), err)]
pub fn index(line_receiver: Receiver<ChapterLine>) -> Result<(), ao3fti_common::Report> {
    let num_threads = 3;
    let memory_size = 1000000000;
    let buffer_size_per_thread = memory_size / num_threads;

    let (doc_sender, doc_receiver) = channel::bounded(10_000);

    let index = Index::open_in_dir("./chapter-index")?;
    let schema = index.schema();

    let num_threads_to_parse_json = std::cmp::max(1, num_threads / 4);
    tracing::info!("Using {} threads to parse json", num_threads_to_parse_json);
    for i in 0..num_threads_to_parse_json {
        let schema_clone = schema.clone();
        let doc_sender_clone = doc_sender.clone();
        let line_receiver_clone = line_receiver.clone();

        let child_span = tracing::debug_span!("child", thread_id = i).or_current();
        std::thread::spawn(move || {
            let _entered = child_span.entered();

            for article_line in line_receiver_clone {
                let article_line = serde_json::to_string(&article_line).unwrap();

                match schema_clone.parse_document(&article_line) {
                    Ok(doc) => {
                        if let Err(err) = doc_sender_clone.send(doc) {
                            tracing::error!(err = ?err, "unable to send document to be indexed");
                        }
                    }
                    Err(err) => {
                        tracing::error!("Failed to add document doc {:?}", err);
                    }
                }
            }
        });
    }
    drop(doc_sender);

    let mut index_writer = if num_threads > 0 {
        index.writer_with_num_threads(num_threads, buffer_size_per_thread)
    } else {
        index.writer(buffer_size_per_thread)
    }?;

    let index_result = index_documents(&mut index_writer, doc_receiver);

    match index_result {
        Ok(docstamp) => {
            tracing::info!("Commit succeed, docstamp at {}", docstamp);
            tracing::info!("Waiting for merging threads");

            index_writer.wait_merging_threads()?;

            tracing::info!("Terminated successfully!");

            Ok(())
        }
        Err(e) => {
            tracing::error!("Error during indexing, rollbacking.");

            index_writer.rollback()?;

            tracing::info!("Rollback succeeded");

            Err(e.into())
        }
    }
}

#[tracing::instrument(skip(index_writer, doc_receiver), err)]
fn index_documents(
    index_writer: &mut IndexWriter,
    doc_receiver: channel::Receiver<Document>,
) -> tantivy::Result<u64> {
    let group_count = 100_000;

    for (num_docs, doc) in doc_receiver.into_iter().enumerate() {
        index_writer.add_document(doc)?;

        if num_docs > 0 && (num_docs % group_count == 0) {
            tracing::info!("{} Docs", num_docs);
        }
    }

    index_writer.commit()
}

pub struct IndexServer {
    pub reader: IndexReader,
    pub query_parser: QueryParser,
    pub schema: Schema,
}

impl IndexServer {
    pub fn new() -> Result<Arc<Self>, ao3fti_common::Report> {
        let index = Index::open_in_dir("chapter-index")?;
        let schema = index.schema();
        let default_fields: Vec<Field> = schema
            .fields()
            .filter(|&(_, field_entry)| match field_entry.field_type() {
                FieldType::Str(ref text_field_options) => {
                    text_field_options.get_indexing_options().is_some()
                }
                _ => false,
            })
            .map(|(field, _)| field)
            .collect();
        let query_parser =
            QueryParser::new(schema.clone(), default_fields, index.tokenizers().clone());
        let reader = index.reader()?;

        let index_server = Arc::new(IndexServer {
            reader,
            query_parser,
            schema,
        });

        Ok(index_server)
    }
}

#[derive(Debug, serde::Serialize)]
pub struct Serp {
    pub query: String,
    pub num_hits: usize,
    pub hits: Vec<Hit>,
    pub timings: TimerTree,
}

#[derive(Debug, serde::Serialize)]
pub struct Hit {
    pub score: Score,
    pub doc: NamedFieldDocument,
    pub id: u32,
}

#[derive(Debug, serde::Deserialize)]
pub struct SearchQuery {
    pub query: String,
    pub offset: usize,
    pub limit: usize,
}

pub fn serp(index: Arc<IndexServer>, search: SearchQuery) -> Result<Serp, ao3fti_common::Report> {
    let searcher = index.reader.searcher();
    let mut timer_tree = TimerTree::default();

    let SearchQuery {
        query: q,
        offset,
        limit,
    } = search;

    let query = index.query_parser.parse_query(&q)?;

    let (top_docs, num_hits) = {
        let _search_timer = timer_tree.open("search");

        searcher.search(
            &query,
            &(TopDocs::with_limit(limit).and_offset(offset), Count),
        )?
    };

    let hits: Vec<Hit> = {
        let _fetching_timer = timer_tree.open("fetching docs");

        top_docs
            .iter()
            .map(|(score, doc_address)| {
                let doc: Document = searcher.doc(*doc_address).unwrap();

                Hit {
                    score: *score,
                    doc: index.schema.to_named_doc(&doc),
                    id: doc_address.doc_id,
                }
            })
            .collect()
    };

    Ok(Serp {
        query: q,
        num_hits,
        hits,
        timings: timer_tree,
    })
}
