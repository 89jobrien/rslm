//! Notebook state, loop outcomes, and errors for recursive inference.

use thiserror::Error;

#[derive(Debug, Clone)]
pub struct Cell {
    pub script: String,
    pub output: String,
}

#[derive(Debug, Default, Clone)]
pub struct Notebook {
    pub cells: Vec<Cell>,
}

impl Notebook {
    /// Appends an executed script and its output to the notebook.
    pub fn push(&mut self, script: String, output: String) {
        self.cells.push(Cell { script, output });
    }
}

#[derive(Debug)]
pub enum StepResult {
    Continue(String),
    Final(String),
}

#[derive(Debug, Error)]
pub enum RlmError {
    #[error("max depth {0} exceeded")]
    MaxDepthExceeded(usize),
    #[error("max iterations {0} exceeded without final() call")]
    MaxIterationsExceeded(usize),
    #[error("script error: {0}")]
    ScriptError(String),
    #[error("provider error: {0}")]
    ProviderError(#[from] anyhow::Error),
}
