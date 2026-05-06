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
