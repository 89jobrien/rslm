use std::sync::{Arc, Mutex};

use anyhow::Result;
use rhai::{Engine, Scope};

/// State shared between Rhai callbacks and the outer loop.
#[derive(Debug, Default, Clone)]
pub struct CallState {
    pub final_answer: Option<String>,
    pub cell_output: Vec<String>,
}

/// Build a Rhai engine with the RLM function set registered.
/// `ctx` is the context string the model can query.
/// `rlm_call_fn` is a blocking callback invoked when the script calls `rlm_call(q, c)`.
pub fn build_engine(
    ctx: String,
    state: Arc<Mutex<CallState>>,
    rlm_call_fn: impl Fn(String, String) -> String + Send + Sync + 'static,
) -> Engine {
    let mut engine = Engine::new();

    // Disable (potentially dangerous) stdlib IO
    engine.set_max_operations(1_000_000);

    // ctx_len
    {
        let ctx = ctx.clone();
        engine.register_fn("ctx_len", move || ctx.len() as i64);
    }

    // ctx_slice(start, end) -> String
    {
        let ctx = ctx.clone();
        engine.register_fn("ctx_slice", move |start: i64, end: i64| {
            let s = start.max(0) as usize;
            let e = (end as usize).min(ctx.len());
            if s >= ctx.len() || s >= e {
                return String::new();
            }
            // slice at char boundaries
            let _bytes = ctx.as_bytes();
            let mut cs = s;
            while cs < ctx.len() && !ctx.is_char_boundary(cs) {
                cs += 1;
            }
            let mut ce = e;
            while ce < ctx.len() && !ctx.is_char_boundary(ce) {
                ce += 1;
            }
            ctx[cs..ce].to_string()
        });
    }

    // ctx_grep(pattern) -> String
    {
        let ctx = ctx.clone();
        engine.register_fn("ctx_grep", move |pattern: &str| -> String {
            match regex::Regex::new(pattern) {
                Ok(re) => ctx
                    .lines()
                    .filter(|l| re.is_match(l))
                    .collect::<Vec<_>>()
                    .join("\n"),
                Err(e) => format!("regex error: {e}"),
            }
        });
    }

    // print_cell(msg)
    {
        let state = state.clone();
        engine.register_fn("print_cell", move |msg: &str| {
            if let Ok(mut s) = state.lock() {
                s.cell_output.push(msg.to_string());
            }
        });
    }

    // final(answer)
    {
        let state = state.clone();
        engine.register_fn("final_answer", move |answer: &str| {
            if let Ok(mut s) = state.lock() {
                s.final_answer = Some(answer.to_string());
            }
        });
    }

    // rlm_call(query, ctx) -> String
    {
        engine.register_fn("rlm_call", move |query: &str, sub_ctx: &str| -> String {
            rlm_call_fn(query.to_string(), sub_ctx.to_string())
        });
    }

    engine
}

#[cfg(feature = "store")]
pub fn register_store_fns(
    engine: &mut rhai::Engine,
    store: std::sync::Arc<rslm_store::ChunkStore>,
    embedder: Option<std::sync::Arc<dyn rslm_store::EmbedProvider>>,
    doc_id: String,
) {
    // doc_id() -> String
    {
        let id = doc_id.clone();
        engine.register_fn("doc_id", move || id.clone());
    }
    // ctx_chunks(doc_id) -> int
    {
        let s = store.clone();
        engine.register_fn("ctx_chunks", move |did: &str| -> i64 {
            s.get_chunks(did).map(|v| v.len() as i64).unwrap_or(0)
        });
    }
    // ctx_chunk(doc_id, i) -> String
    {
        let s = store.clone();
        engine.register_fn("ctx_chunk", move |did: &str, i: i64| -> String {
            s.get_chunks(did)
                .ok()
                .and_then(|v| v.into_iter().nth(i as usize).map(|(_, t)| t))
                .unwrap_or_default()
        });
    }
    // ctx_search(doc_id, query, k) -> String — BM25
    {
        let s = store.clone();
        engine.register_fn("ctx_search", move |did: &str, query: &str, k: i64| -> String {
            let chunks = s.get_chunks(did).unwrap_or_default();
            let indexed: Vec<(usize, &str)> = chunks.iter().map(|(i, t)| (*i, t.as_str())).collect();
            let idx = rslm_store::bm25::Bm25Index::build(&indexed);
            idx.search(query, k as usize)
                .into_iter()
                .filter_map(|(id, _)| chunks.iter().find(|(i, _)| *i == id).map(|(_, t)| t.clone()))
                .collect::<Vec<_>>()
                .join("\n---\n")
        });
    }
    // ctx_hybrid(doc_id, query, k) -> String — sync wrapper, runs new runtime
    {
        let s = store.clone();
        let emb = embedder.clone();
        engine.register_fn("ctx_hybrid", move |did: &str, query: &str, k: i64| -> String {
            let store2 = s.clone();
            let emb2 = emb.clone();
            let did2 = did.to_string();
            let query2 = query.to_string();
            let k2 = k as usize;
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("build hybrid runtime");
            rt.block_on(async move {
                // embed query if embedder available
                let query_vec: Vec<f32> = if let Some(ref e) = emb2 {
                    e.embed(vec![query2.clone()]).await.ok()
                        .and_then(|v| v.into_iter().next())
                        .unwrap_or_default()
                } else {
                    vec![]
                };

                if query_vec.is_empty() {
                    // fall back to BM25 only
                    let chunks = store2.get_chunks(&did2).unwrap_or_default();
                    let indexed: Vec<(usize, &str)> =
                        chunks.iter().map(|(i, t)| (*i, t.as_str())).collect();
                    let idx = rslm_store::bm25::Bm25Index::build(&indexed);
                    idx.search(&query2, k2)
                        .into_iter()
                        .filter_map(|(id, _)| {
                            chunks.iter().find(|(i, _)| *i == id).map(|(_, t)| t.clone())
                        })
                        .collect::<Vec<_>>()
                        .join("\n---\n")
                } else {
                    store2
                        .search_hybrid(&did2, &query2, &query_vec, k2)
                        .await
                        .map(|v| v.into_iter().map(|(_, t)| t).collect::<Vec<_>>().join("\n---\n"))
                        .unwrap_or_default()
                }
            })
        });
    }
}

pub fn run_script(
    engine: &Engine,
    script: &str,
    state: &Arc<Mutex<CallState>>,
) -> Result<String, String> {
    // Clear per-cell output
    if let Ok(mut s) = state.lock() {
        s.cell_output.clear();
    }

    let mut scope = Scope::new();
    match engine.eval_with_scope::<rhai::Dynamic>(&mut scope, script) {
        Ok(val) => {
            let mut output_parts: Vec<String> = vec![];
            if let Ok(s) = state.lock() {
                output_parts.extend(s.cell_output.clone());
            }
            let val_str = val.to_string();
            if val_str != "()" {
                output_parts.push(val_str);
            }
            Ok(output_parts.join("\n"))
        }
        Err(e) => Err(e.to_string()),
    }
}
