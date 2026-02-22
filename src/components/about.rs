use leptos::prelude::*;

#[component]
pub fn About() -> impl IntoView {
    view! {
        <div class="max-w-3xl flex flex-col gap-6 text-base leading-loose text-charcoal">
            <p>
                "You're on some old computers and Raspberry Pis in my closet. This site runs on a self-hosted "
                <a href="https://kubernetes.io/" target="_blank" class="text-accent hover:underline transition-colors duration-200">
                    Kubernetes
                </a>
                " cluster — autoscaling "
                <a href="https://github.com/actions/actions-runner-controller" target="_blank" class="text-accent hover:underline transition-colors duration-200">
                    GitHub Actions runners
                </a>
                " (x86 + arm), in-cluster "
                <a href="https://goharbor.io/" target="_blank" class="text-accent hover:underline transition-colors duration-200">
                    Harbor
                </a>
                " registry, "
                <a href="https://rook.io/" target="_blank" class="text-accent hover:underline transition-colors duration-200">
                    Rook-Ceph
                </a>
                " storage, built with "
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
                "."
            </p>
            <p>
                "I'm a software engineer at "
                <a href="https://www.astranis.com/" target="_blank" class="text-accent hover:underline transition-colors duration-200">
                    "Astranis"
                </a>
                ", building satellites. I work on everything from UI and monitoring for Satcom payloads to running production clusters and planning disaster recovery."
            </p>
        </div>
    }
}
