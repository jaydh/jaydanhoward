use crate::components::source_anchor::SourceAnchor;
use leptos::ev::{MouseEvent, TouchEvent, WheelEvent};
use leptos::*;

#[component]
pub fn PictureSection(cx: Scope) -> impl IntoView {
    view! { cx,
        <div class="w-fit max-h-3xl relative">
            <img src="/assets/profile.webp" class="filter grayscale opacity-50 object-cover"/>
            <div class="absolute left-0 w-full h-40 bottom-0">
                <div class="h-full w-full bg-gradient-to-b from-transparent to-charcoal"></div>
            </div>
            <div class="text-xl text-white absolute top-1/4 left-1/2 transform -translate-x-1/2 -translate-y-1/2">
                "I'm Jay Dan Howard!"
            </div>
            <div class="absolute top-3/4 left-1/2 transform -translate-x-1/2 -translate-y-1/2">
                I believe compassion makes tech worthwhile
            </div>
        </div>
    }
}

#[component]
pub fn MeSection(cx: Scope) -> impl IntoView {
    view! { cx,
        <p>"Very few things are good in and of themselves, and tech is probably not one of them"</p>
        <p>
            "I'm currently a senior software engineer at Interwell Health, leading an engineering team where we use software to empower clinicians and nephrologists to treat and prevent kidney disease"
        </p>
        <p>
            "I try to keep a low-key life and avoid the spotlight but with that said, I plan to change the world."
        </p>
    }
}

#[component]
pub fn SiteSection(cx: Scope) -> impl IntoView {
    view! { cx,
        <p>
            "This site exists to experiment with tech (currently that's Rust + Leptos + Tailwind), and to have a small corner of the internet where people can learn about me
            (mostly in a software engineering context)"
        </p>
        <p>"I live in beautiful San Francisco"</p>
        <p>
            "I spend my AFK time walking my dog Lunabelle, wrenching on my motorcycle, and mindfully engaging in silliness"
        </p>
    }
}

#[component]
pub fn ShowWithTransition<W>(cx: Scope, children: ChildrenFn, when: W) -> impl IntoView
where
    W: Fn() -> bool + 'static,
{
    let memoized_when = create_memo(cx, move |_| when());

    view! { cx,
        <div
            class="transition-all duration-3000 transform scale-y-0 opacity-0"
            class=("opacity-100", move || memoized_when() == true)
            class=("scale-y-100", move || memoized_when() == true)
            class=("hidden", move || memoized_when() == false)
        >
            {children(cx)}
        </div>
    }
}

#[component]
pub fn About(cx: Scope) -> impl IntoView {
    let section_length = 3;
    let (section, set_section) = create_signal(cx, 0);
    let (touch_start_y, set_touch_start_y) = create_signal(cx, -1);

    let arrow_class = move || match section() < section_length - 1 {
        true => "fas fa-chevron-down cursor-pointer",
        false => "fas fa-chevron-up cursor-pointer",
    };

    let handle_scroll = move |e: WheelEvent| {
        if e.delta_y() > 0.0 && section() < section_length - 1 {
            set_section.set(section() + 1);
        } else if e.delta_y() < 0.0 && section() > 0 {
            set_section.set(section() - 1);
        }
    };

    let handle_next = move |_e: MouseEvent| match arrow_class().contains("up") {
        true => set_section.set(section() - 1),
        false => set_section.set(section() + 1),
    };

    let handle_touch_start = move |e: TouchEvent| match e.touches().item(0) {
        Some(touchStart) => set_touch_start_y(touchStart.client_y()),
        None => set_touch_start_y(-1),
    };

    let handle_touch_move = move |e: TouchEvent| match e.touches().item(0) {
        Some(touchEnd) => {
            if touch_start_y() > -1 {
                let delta_y = touchEnd.client_y() - touch_start_y();
                match delta_y {
                    d if d.abs() < 50 => {}
                    d if d < 0 && section() < section_length - 1 => {
                        set_section.set(section() + 1);
                        set_touch_start_y(-1);
                    }
                    d if d > 0 && section() > 0 => {
                        set_section.set(section() - 1);
                        set_touch_start_y(-1);
                    }
                    _ => {}
                }
            }
        }
        None => set_touch_start_y(-1),
    };

    view! { cx,
        <SourceAnchor href="#[git]"/>
        <div
            class="flex flex-col text-white text-lg w-1/2 max-w-xl scroll-smooth"
            on:wheel=handle_scroll
            on:touchstart=handle_touch_start
            on:touchmove=handle_touch_move
        >
            <div class="flex flex-col space-y-10 max-w-lg">
                <ShowWithTransition when=move || { section() == 0 }>
                    <PictureSection/>
                </ShowWithTransition>
                <ShowWithTransition when=move || { section() == 1 }>
                    <MeSection/>
                </ShowWithTransition>
                <ShowWithTransition when=move || { section() == 2 }>
                    <SiteSection/>
                </ShowWithTransition>
                <div class="text-xl flex flex-col items-center">
                    <i class=arrow_class on:click=handle_next></i>
                </div>
            </div>
        </div>
    }
}
