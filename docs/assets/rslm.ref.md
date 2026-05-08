# rslm — Reference Documentation
#
# Recursive Language Model inference engine (Rust).
# Based on Zhang & Khattab, 2025: https://alexzhang13.github.io/blog/2025/rlm/
#
# Sections:
#   1. CLI commands          — rslm binary interface
#   2. Rhai built-ins        — functions available inside every RLM script
#   3. Store Rhai built-ins  — additional functions when --store is active
#   4. Rust API              — rslm-core and rslm-providers public surface
#   5. Environment variables — runtime configuration
#   6. Error types           — RlmError variants
#   7. Workflow patterns     — canonical Rhai script patterns

# ─────────────────────────────────────────────────────────────────────────────
# 1. CLI COMMANDS
# ─────────────────────────────────────────────────────────────────────────────

# Usage: rslm query <QUERY> [--context-file <PATH>] [--context <STRING>]
#        [--provider openai|anthropic] [--model <ID>]
#        [--max-depth <N>] [--max-iterations <N>]
#        [--verbose]
#        [--store <PATH>] [--doc-id <ID>] [--embed-model <ID>]
#
# Run a single query against a context. The context is read from --context-file
# or supplied inline with --context. If neither is given, context is empty.
#
# When --store is provided the context is ingested into the SQLite chunk store
# (if not already present for the resolved doc-id) and the store Rhai functions
# become available to the model.
#
# Example:
#
#   rslm query "Who wrote the preface?" --context-file book.txt
#
#   rslm query "Summarise chapter 3" \
#       --context-file book.txt \
#       --store book.db \
#       --provider anthropic \
#       --verbose
#
rslm_query() { :; }

# Usage: rslm interactive
#        [--provider openai|anthropic] [--model <ID>]
#        [--max-depth <N>] [--max-iterations <N>] [--verbose]
#
# Launch an interactive REPL. Each iteration prompts for a query and a context
# string. Useful for exploratory work without shell quoting overhead.
#
# Example:
#
#   rslm interactive --verbose
#
rslm_interactive() { :; }

# Usage: rslm ingest --context-file <PATH>
#        --store <PATH>
#        [--strategy fixed|paragraph|line]
#        [--doc-id <ID>]
#        [--embed-model <ID>]
#
# Ingest a document into the SQLite chunk store. Chunks are split by the chosen
# strategy and, if OPENAI_API_KEY is present, embedded for hybrid search.
#
# --strategy options:
#   fixed      — fixed-size byte windows
#   paragraph  — split on blank lines (default)
#   line       — one chunk per line
#
# Example:
#
#   rslm ingest --context-file book.txt --store book.db --strategy paragraph
#
#   rslm ingest --context-file paper.pdf.txt \
#       --store research.db \
#       --doc-id paper-2025 \
#       --embed-model text-embedding-3-large
#
rslm_ingest() { :; }

# ─────────────────────────────────────────────────────────────────────────────
# 2. RHAI BUILT-INS
# ─────────────────────────────────────────────────────────────────────────────
#
# These functions are registered in every Rhai execution scope.
# The model's response for each iteration MUST be a valid Rhai script.
# No markdown fences. No prose. Pure Rhai.
#
# Each script runs in a fresh Scope::new() — no variables persist between
# iterations.

# Usage: ctx_len() -> int
#
# Returns the total byte length of the context string passed to this RLM
# instance. Useful for deciding whether to slice or grep.
#
# Example:
#
#   let n = ctx_len();
#   if n < 4096 {
#       final_answer(ctx_slice(0, n));
#   }
#
ctx_len() { :; }

# Usage: ctx_slice(start: int, end: int) -> String
#
# Returns the byte range [start, end) of the context, automatically snapped to
# valid UTF-8 character boundaries. Returns an empty string if the range is
# out of bounds or inverted.
#
# Parameters:
#   start — inclusive byte offset (clamped to 0)
#   end   — exclusive byte offset (clamped to ctx_len())
#
# Example:
#
#   let header = ctx_slice(0, 512);
#   final_answer(header);
#
#   // Page through a large context in 2 KB windows
#   let page = ctx_slice(2048, 4096);
#
ctx_slice() { :; }

# Usage: ctx_grep(pattern: String) -> String
#
# Searches the context line-by-line using a Rust regex. Returns all matching
# lines joined by newline. Returns a "regex error: …" string if the pattern
# is invalid — never panics.
#
# Rules:
# - You MUST call ctx_grep or ctx_slice at least once before final_answer.
# - If the result is empty, try alternative patterns before giving up.
#
# Example:
#
#   let hits = ctx_grep("(?i)introduction");
#   final_answer(hits);
#
#   // Case-insensitive word boundary search
#   let dates = ctx_grep("\\b20[0-9]{2}\\b");
#
ctx_grep() { :; }

# Usage: print_cell(msg: String)
#
# Appends msg to this cell's output buffer. The accumulated output is returned
# to the model as "Cell output: …" after the script completes. Use for
# intermediate logging or multi-value inspection.
#
# Example:
#
#   print_cell("scanning for author …");
#   let r = ctx_grep("Author:");
#   print_cell(r);
#   final_answer(r);
#
print_cell() { :; }

# Usage: final_answer(answer: String)
#
# Signals the final answer and exits the RLM loop. The string becomes the
# return value of Rlm::run(). Must be called with a non-empty, specific answer
# derived from context results.
#
# Rules:
# - Never call final_answer("") — the loop will reject empty answers.
# - Never call final_answer with a vague "not found" message without first
#   trying ctx_slice to scan the raw context.
# - Only one final_answer call per script is meaningful; subsequent calls are
#   ignored.
#
# Example:
#
#   let r = ctx_grep("license");
#   final_answer(r);
#
final_answer() { :; }

# Usage: rlm_call(query: String, ctx: String) -> String
#
# Spawns a child RLM instance at depth+1 with its own isolated scope and the
# given sub-context. Blocks until the child produces a final answer or errors.
# Returns the child's answer string, or "rlm_call error: …" on failure.
#
# The child inherits the same provider, max_depth, and max_iterations as the
# parent. Recursion terminates when depth reaches max_depth (default 5).
#
# Example:
#
#   // Split the context and delegate each half
#   let mid = ctx_len() / 2;
#   let left  = ctx_slice(0, mid);
#   let right = ctx_slice(mid, ctx_len());
#   let a1 = rlm_call("find the date", left);
#   let a2 = rlm_call("find the date", right);
#   final_answer(a1 + " | " + a2);
#
rlm_call() { :; }

# ─────────────────────────────────────────────────────────────────────────────
# 3. STORE RHAI BUILT-INS  (requires --store)
# ─────────────────────────────────────────────────────────────────────────────
#
# Available when rslm is invoked with --store <PATH>. The store is a SQLite
# database of pre-chunked and optionally embedded document content.

# Usage: doc_id() -> String
#
# Returns the document ID used for this query. Equivalent to the --doc-id CLI
# flag or the resolved default (context file path or "default").
#
# Example:
#
#   let id = doc_id();
#   let n = ctx_chunks(id);
#
doc_id() { :; }

# Usage: ctx_chunks(doc_id: String) -> int
#
# Returns the number of chunks stored for the given document ID. Returns 0 if
# the document is not found.
#
# Example:
#
#   let n = ctx_chunks(doc_id());
#   print_cell("total chunks: " + n);
#
ctx_chunks() { :; }

# Usage: ctx_chunk(doc_id: String, i: int) -> String
#
# Returns the i-th chunk (0-indexed) for the given document ID. Returns an
# empty string if i is out of range.
#
# Example:
#
#   let first = ctx_chunk(doc_id(), 0);
#   final_answer(first);
#
ctx_chunk() { :; }

# Usage: ctx_search(doc_id: String, query: String, k: int) -> String
#
# BM25 keyword search over the stored chunks. Returns the top-k matching chunks
# joined by "\n---\n". Use when semantic similarity is not needed or when no
# embedder is configured.
#
# Example:
#
#   let results = ctx_search(doc_id(), "transformer architecture", 5);
#   final_answer(results);
#
ctx_search() { :; }

# Usage: ctx_hybrid(doc_id: String, query: String, k: int) -> String
#
# Hybrid semantic + BM25 search using Reciprocal Rank Fusion (RRF). Requires
# OPENAI_API_KEY to be set and an embedder configured. Falls back to BM25-only
# if no embedder is available.
#
# Preferred over ctx_search when the query is conceptual or paraphrased.
#
# Example:
#
#   let results = ctx_hybrid(doc_id(), "model recursion depth limit", 8);
#   final_answer(results);
#
ctx_hybrid() { :; }

# ─────────────────────────────────────────────────────────────────────────────
# 4. RUST API
# ─────────────────────────────────────────────────────────────────────────────

# Usage: Rlm::new(provider, max_depth, max_iterations, verbose) -> Rlm
#
# Constructs a new RLM engine instance. provider must implement LlmProvider.
#
# Parameters:
#   provider        — Arc<dyn LlmProvider>
#   max_depth       — maximum recursive rlm_call depth (5 is a safe default)
#   max_iterations  — maximum Rhai-execute iterations per instance (20 default)
#   verbose         — print depth indicators and cell outputs to stdout
#
# Example:
#
#   let provider = Arc::new(OpenAiProvider::new("gpt-4o"));
#   let rlm = Rlm::new(provider, 5, 20, false);
#
Rlm_new() { :; }

# Usage: Rlm::with_store(store, embedder, doc_id) -> Rlm
#
# Attaches a ChunkStore to the RLM instance, enabling the store Rhai built-ins.
# Only available with the "store" feature flag.
#
# Parameters:
#   store    — Arc<ChunkStore>
#   embedder — Option<Arc<dyn EmbedProvider>>
#   doc_id   — String identifying the document in the store
#
# Example:
#
#   let store = Arc::new(ChunkStore::open("book.db")?);
#   let rlm = Rlm::new(provider, 5, 20, false)
#       .with_store(Arc::clone(&store), None, "book.txt".to_string());
#
Rlm_with_store() { :; }

# Usage: Rlm::run(query, ctx) -> Result<String, RlmError>
#
# Executes the RLM loop for the given query and context string. Returns the
# model's final answer or an RlmError.
#
# This is an async fn — call it from a tokio runtime or with .await.
#
# Example:
#
#   let answer = rlm.run("Who wrote the introduction?", &ctx).await?;
#   println!("{answer}");
#
Rlm_run() { :; }

# Usage: LlmProvider trait
#
# Implement this trait to add a new model backend.
#
#   async fn complete(&self, messages: Vec<Message>) -> Result<String>;
#   fn model_id(&self) -> &str;
#
# Built-in implementations:
#   OpenAiProvider::new(model_id: &str)     — reads OPENAI_API_KEY
#   AnthropicProvider::new(model_id: &str)  — reads ANTHROPIC_API_KEY
#
# Example:
#
#   let p = Arc::new(AnthropicProvider::new("claude-sonnet-4-6")?);
#
LlmProvider_trait() { :; }

# Usage: ChunkStore::open(path: &str) -> Result<ChunkStore>
#
# Opens (or creates) the SQLite chunk store at the given path.
#
# Example:
#
#   let store = ChunkStore::open("research.db")?;
#
ChunkStore_open() { :; }

# Usage: ChunkStore::ingest(doc_id, text, strategy, embedder) -> Result<()>
#
# Chunks text according to strategy and stores the chunks. If an embedder is
# provided, also computes and stores embeddings for each chunk.
#
# ChunkStrategy variants: Fixed(usize), Paragraph, Line
#
# Example:
#
#   store.ingest("doc1", &text, &ChunkStrategy::Paragraph, None).await?;
#
ChunkStore_ingest() { :; }

# ─────────────────────────────────────────────────────────────────────────────
# 5. ENVIRONMENT VARIABLES
# ─────────────────────────────────────────────────────────────────────────────

# RSLM_PROVIDER=openai|anthropic
#
# Selects the LLM provider. Overridden by --provider CLI flag.
# Default: openai

# RSLM_MODEL=<model-id>
#
# Selects the model within the chosen provider. Overridden by --model CLI flag.
# Defaults: gpt-4o (openai), claude-sonnet-4-6 (anthropic)

# OPENAI_API_KEY=<key>
#
# Required for OpenAI provider and for OpenAI-based embedding (ctx_hybrid).

# ANTHROPIC_API_KEY=<key>
#
# Required for Anthropic provider.

# RUST_LOG=<filter>
#
# Controls tracing output (written to stderr). Examples:
#   RUST_LOG=debug          — all crates at debug level
#   RUST_LOG=rslm_core=info — rslm-core at info only

# ─────────────────────────────────────────────────────────────────────────────
# 6. ERROR TYPES
# ─────────────────────────────────────────────────────────────────────────────

# RlmError::MaxDepthExceeded(usize)
#
# Returned when rlm_call is invoked and depth >= max_depth. Indicates the
# recursion budget was exhausted. Increase --max-depth or restructure the
# query to require fewer recursive delegations.

# RlmError::MaxIterationsExceeded(usize)
#
# Returned when the model reaches max_iterations without calling final_answer.
# The model is looping or not converging. Try --verbose to inspect the trace,
# or increase --max-iterations.

# RlmError::ScriptError(String)
#
# Returned after MAX_ERROR_STREAK (3) consecutive invalid Rhai scripts from the
# model. The model could not self-correct. The message includes the last error
# and the offending script fragment.

# RlmError::ProviderError(anyhow::Error)
#
# Wraps any error from the LlmProvider::complete call (network, auth, rate
# limit, etc.). Check OPENAI_API_KEY / ANTHROPIC_API_KEY first.

# ─────────────────────────────────────────────────────────────────────────────
# 7. WORKFLOW PATTERNS
# ─────────────────────────────────────────────────────────────────────────────

# Pattern: grep-then-answer
#
# The minimal correct pattern. Always grep before answering.
#
#   let r = ctx_grep("(?i)author");
#   final_answer(r);

# Pattern: multi-grep with fallback
#
# Try progressively broader patterns before giving up.
#
#   let r = ctx_grep("(?i)written by");
#   if r == "" {
#       r = ctx_grep("(?i)author");
#   }
#   if r == "" {
#       r = ctx_slice(0, 512);
#   }
#   final_answer(r);

# Pattern: page-through context
#
# Scan a large context in windows when grep returns nothing.
#
#   let step = 2048;
#   let total = ctx_len();
#   let i = 0;
#   let found = "";
#   while i < total && found == "" {
#       let page = ctx_slice(i, i + step);
#       if page.contains("Chapter 3") {
#           found = page;
#       }
#       i += step;
#   }
#   final_answer(if found != "" { found } else { ctx_slice(0, 512) });

# Pattern: recursive split-and-delegate
#
# Divide and conquer for large contexts that exceed a single model's attention.
#
#   let mid = ctx_len() / 2;
#   let a = rlm_call("find the publication date", ctx_slice(0, mid));
#   let b = rlm_call("find the publication date", ctx_slice(mid, ctx_len()));
#   final_answer(a + " | " + b);

# Pattern: store hybrid search
#
# Preferred for large ingested documents when --store is active.
#
#   let id = doc_id();
#   let results = ctx_hybrid(id, "transformer attention mechanism", 6);
#   final_answer(results);

# Pattern: chunk enumeration
#
# Iterate chunks when keyword and semantic search both fail.
#
#   let id = doc_id();
#   let n = ctx_chunks(id);
#   let i = 0;
#   let found = "";
#   while i < n && found == "" {
#       let c = ctx_chunk(id, i);
#       if c.contains("license") {
#           found = c;
#       }
#       i += 1;
#   }
#   final_answer(if found != "" { found } else { "not located in chunks" });
