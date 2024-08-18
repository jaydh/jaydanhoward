use crate::components::cluster_stats::ClusterStats;
use crate::components::source_anchor::SourceAnchor;

use leptos::*;

#[component]
pub fn PictureSection() -> impl IntoView {
    view! {
        <img
            src="/assets/profile.webp"
            class="h-auto max-w-full filter grayscale dark:opacity-50"
            srcset="/assets/profile-small.webp 320w, /assets/profile-medium.webp 480w, /assets/profile.webp 720w"
            sizes="(max-width:640px) 320px, (max-width:768px) 480px, 720px"
            alt="Picture of me"
            height=672
            width=504
        />
    }
}

#[component]
pub fn MeSection() -> impl IntoView {
    view! {
        <div class="flex flex-row">
            <div class="flex flex-col space-y-10">
                <p>
                    "Thanks for checking out my k8s cluster! Unbeknownst to you, your browser is talking to some old computers and raspberry pis in my closet, thanks for asking. This website and cluster are my pet projects for exploring using "
                    <a href="https://leptos.dev/" target="_blank" class="underline">
                        Rust on the web
                    </a> " and self-hosting "
                    <a href="https://kubernetes.io/" target="_blank" class="underline">
                        Kubernetes
                    </a> " services for my personal use. "
                    <a
                        href="https://developers.cloudflare.com/cloudflare-one/"
                        target="_blank"
                        class="underline"
                    >
                        Cloudflare Tunnels
                    </a>
                    " enable you to securely connect to services on my cluster without me risking my apartment network."
                </p>
                <p>
                    "As my girlfriend puts it: \"You setting up your cluster to use a distributed filesystem for self-hosting your personal Dropbox, container registry, and so on is like putting a rock in a diamond vault. You're never going to watch those motorcycle recordings.\" She's not wrong... but I "
                    <em>"could"</em>", from anywhere. And that's pretty neat."

                </p>
                <p>
                    "I'm currently a software engineer at "
                    <a href="https://www.astranis.com/" target="_blank" class="underline">
                        "Astranis"
                    </a>
                    ", where we make dedicated microgeo satellites. I do everything from building UI and services for monitoring and commanding Satcom payloads, to administering and planning disaster-recovery for our coporate cluster and production databases. I care a lot about writing reliable software, end to end. That involves making sure software runs fast (whereever it runs) and it screams loud when it isn't (in a way that coaches people to pay attention)."
                </p>
            </div>
            <div class="ml-auto">
                <PictureSection />
            </div>
        </div>
    }
}

#[component]
pub fn About() -> impl IntoView {
    view! {
        <SourceAnchor href="#[git]" />
        <ClusterStats />
        <div class="w-5/6 mg:w-4/6 p-10 grow text-2xl flex flex-col">
            <div>
                <MeSection />
            </div>
            <div></div>
        </div>
    }
}
