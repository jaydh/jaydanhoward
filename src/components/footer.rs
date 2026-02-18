use leptos::prelude::*;

#[component]
pub fn Footer() -> impl IntoView {
    let sha = option_env!("GIT_SHA").filter(|s| !s.is_empty());
    let display = sha
        .map(|s| s[..s.len().min(7)].to_string())
        .unwrap_or_else(|| "dev".to_string());

    view! {
        <footer class="border-t border-border py-3 px-8 flex items-center justify-center shrink-0">
            <span class="text-xs text-charcoal/40 font-mono">{display}</span>
        </footer>
    }
}
