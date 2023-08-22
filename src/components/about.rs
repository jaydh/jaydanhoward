use leptos::*;

#[component]
pub fn About(cx: Scope) -> impl IntoView {
    view! {
        cx,
        <div class="flex flex-row">
            <div class="flex flex-col space-y-10">
                <h1 class="text-5xl font-heavy mb-6">r#"👋I'm Jay Dan Howard! I believe compassion makes tech worthwhile."#</h1>
                <p>"Very few things are good in and of themselves, and tech is probably not one of them. I'm currently a senior software engineer at Interwell Health, leading an engineering team where we use software to empower clinicians and nephrologists to treat and prevent kidney disease. I try to keep a low-key life and avoid the spotlight but with that said, I plan to change the world."
                </p>
                <p>
                    r#"This site exists to experiment with tech (currently that's Rust + Leptos + Tailwind), and to have a small corner of the internet where people can learn about me
                        (mostly in a software engineering context). I live in beautiful San Francisco.
                        I spend my AFK time walking my dog Lunabelle, wrenching on my motorcycle, and mindfully engaging in sillyness."#
                </p>
            </div>
            <img src="/assets/profile.jpg" class="pl-20"/>
        </div>

    }
}
