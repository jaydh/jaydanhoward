use crate::components::cluster_stats::ClusterStats;
use crate::components::source_anchor::SourceAnchor;

use leptos::prelude::*;

#[component]
pub fn MeSection() -> impl IntoView {
    view! {
        <div class="max-w-3xl flex flex-col gap-6 text-base leading-loose text-charcoal opacity-90">
            <p>
                "You're currently talking to some old computers and raspberry pis in my closet. This site runs on a self-hosted "
                <a href="https://kubernetes.io/" target="_blank" class="text-accent hover:underline transition-colors duration-200">
                    Kubernetes
                </a>
                " cluster with a full CI/CD pipeline: autoscaling "
                <a href="https://github.com/actions/actions-runner-controller" target="_blank" class="text-accent hover:underline transition-colors duration-200">
                    GitHub Actions runners
                </a>
                " (x86 + arm), in-cluster "
                <a href="https://goharbor.io/" target="_blank" class="text-accent hover:underline transition-colors duration-200">
                    Harbor
                </a>
                " registry, and "
                <a href="https://rook.io/" target="_blank" class="text-accent hover:underline transition-colors duration-200">
                    Rook-Ceph
                </a>
                " distributed storage backing everything. Built with "
                <a href="https://leptos.dev/" target="_blank" class="text-accent hover:underline transition-colors duration-200">
                    Rust/Leptos
                </a>
                ", exposed via "
                <a
                    href="https://developers.cloudflare.com/cloudflare-one/"
                    target="_blank"
                    class="text-accent hover:underline transition-colors duration-200"
                >
                    Cloudflare Tunnels
                </a>
                " so you can reach my apartment network without me risking it."
            </p>
            <p>
                "I'm a software engineer at "
                <a href="https://www.astranis.com/" target="_blank" class="text-accent hover:underline transition-colors duration-200">
                    "Astranis"
                </a>
                ", building satellites. I work on everything from UI and monitoring services for Satcom payloads to administering production clusters and planning disaster recovery. I care about reliable software end-to-end: making it run fast and scream loud when it breaks (in ways that make people pay attention)."
            </p>
        </div>
    }
}

#[component]
pub fn About() -> impl IntoView {
    view! {
        <SourceAnchor href="#[git]" />
        <ClusterStats />
        <div class="max-w-7xl mx-auto px-8 w-full flex justify-center">
            <MeSection />
        </div>
    }
}
