#![allow(clippy::all)]
use crate::components::source_anchor::SourceAnchor;
use leptos::prelude::*;

#[component]
pub fn ClusterStats() -> impl IntoView {
    view! { <SourceAnchor href="#[git]" /> }
}
