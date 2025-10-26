use crate::components::cluster_stats::ClusterStats;
use crate::components::source_anchor::SourceAnchor;

use leptos::prelude::*;

#[component]
pub fn MeSection() -> impl IntoView {
    view! {
        <div class="max-w-3xl flex flex-col gap-6 text-base leading-loose text-charcoal dark:text-gray opacity-90 dark:opacity-85">
            <p>
                "Thanks for checking out my k8s cluster! Unbeknownst to you, your browser is talking to some old computers and raspberry pis in my closet, thanks for asking. This website and cluster are my pet projects for exploring using "
                <a href="https://leptos.dev/" target="_blank" class="text-accent dark:text-accent-light hover:underline transition-colors duration-200">
                    Rust on the web
                </a> " and self-hosting "
                <a href="https://kubernetes.io/" target="_blank" class="text-accent dark:text-accent-light hover:underline transition-colors duration-200">
                    Kubernetes
                </a> ". "
                <a
                    href="https://developers.cloudflare.com/cloudflare-one/"
                    target="_blank"
                    class="text-accent dark:text-accent-light hover:underline transition-colors duration-200"
                >
                    Cloudflare Tunnels
                </a>
                " enable you to securely connect to services on my cluster without me risking my apartment network."
            </p>
            <p>
                "I'm currently a software engineer at "
                <a href="https://www.astranis.com/" target="_blank" class="text-accent dark:text-accent-light hover:underline transition-colors duration-200">
                    "Astranis"
                </a>
                ", where we make dedicated microgeo satellites. I do everything from building UI and services for monitoring and commanding Satcom payloads, to administering and planning disaster-recovery for our coporate cluster and production databases. I care a lot about writing reliable software, end to end. That involves making sure software runs fast (whereever it runs) and it screams loud when it isn't (in a way that coaches people to pay attention)."
            </p>
        </div>
    }
}

#[component]
pub fn About() -> impl IntoView {
    view! {
        <SourceAnchor href="#[git]" />
        <ClusterStats />
        <div class="max-w-7xl mx-auto px-8 py-16 w-full flex justify-center">
            <MeSection />
        </div>
    }
}
