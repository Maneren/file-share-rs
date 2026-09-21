//! Upload progress state shared between [`super::FileUpload`],
//! [`super::progress_bar::ProgressBar`] and [`super::progress_hook`].

use std::collections::VecDeque;

use leptos::prelude::*;
use web_time::Instant;

#[derive(Debug, Clone, Copy)]
pub struct Progress {
    pub size: u64,
    pub start_time: Instant,
    pub uploaded: RwSignal<VecDeque<(u64, Instant)>>,
}
