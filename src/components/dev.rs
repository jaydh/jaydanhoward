use crate::components::source_anchor::SourceAnchor;
use leptos::prelude::*;
use runfiles::{rlocation, Runfiles};

#[component]
pub fn Lighthouse() -> impl IntoView {
    let r = Runfiles::create().expect("Must run using bazel with runfiles");
    let assets_path = rlocation!(r, "_main/assets").expect("Failed to locate main");

    view! {
        <div>
            "Here is a programmatically generated lighthouse report "
            <a
                class="font-semibold underline"
                href="https://github.com/jaydh/jaydanhoward/blob/main/lighthouse/entrypoint.sh"
                target="_blank"
                rel="noreferrer"
            >
                for every deploy
            </a> "that gets kicked off as part of a k8s job for every new deploy of this site."
        </div>
        <iframe
            src=format!("{}/lighthouse.html", assets_path.to_string_lossy())
            title="Lighthouse Report"
            class="grow w-full"
        ></iframe>
    }
}

#[component]
pub fn Dev() -> impl IntoView {
    view! {
        <SourceAnchor href="#[git]" />
        <Lighthouse />
    }
}
