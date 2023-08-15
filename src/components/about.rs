use leptos::*;

#[component]
pub fn About(cx: Scope) -> impl IntoView {
    view! {
        cx,
        <div class="flex flex-row">
            <div>
                <h1 class="text-5xl font-heavy mb-6">r#"👋I'm Jay Dan Howard! I believe compassion makes tech worthwhile."#</h1>
                <p>"Very few things are good in and of themselves, and tech is probably not one of them. I'm currently a senior software engineer at Interwell Health, leading an engineering team where we use software to empower clinicians and nephrologists to treat and prevent kidney disease. I try to keep a low-key life and avoid the spotlight but with that said, I plan to change the world."
                </p>
            </div>
            <img src="/assets/profile.jpg" class="pl-20"/>
        </div>

    }
}
