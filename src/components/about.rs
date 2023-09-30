use crate::components::binary_nav_switcher::BinaryNavSwithcer;
use crate::components::source_anchor::SourceAnchor;
use leptos::ev::{TouchEvent, WheelEvent};
use leptos::*;
use leptos_router::{use_navigate, use_params_map, Outlet};

#[component]
pub fn PictureSection() -> impl IntoView {
    view! {
        <img
            src="/assets/profile.webp"
            class="grow h-auto max-w-full filter grayscale opacity-50 object-cover"
        />
        <div class="grow absolute bottom-0 left-0 w-full h-20 ">
            <div class="h-full w-full bg-gradient-to-b from-transparent to-charcoal"></div>
        </div>
        <div class="text-xl text-white absolute top-1/4 left-1/2 transform -translate-x-1/2 -translate-y-1/2">
            "I'm Jay Dan Howard!"
        </div>
        <div class="absolute top-3/4 left-1/2 transform -translate-x-1/2 -translate-y-1/2">
            I believe compassion makes tech worthwhile
        </div>
    }
}

#[component]
pub fn MeSection() -> impl IntoView {
    view! {
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
pub fn SiteSection() -> impl IntoView {
    view! {
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
pub fn ShowWithTransition<W>(children: ChildrenFn, when: W) -> impl IntoView
where
    W: Fn() -> bool + 'static,
{
    let memoized_when = create_memo(move |_| when());

    view! {
        <div
            class="transition-all duration-3000 transform scale-y-0 opacity-0"
            class=("opacity-100", move || memoized_when() == true)
            class=("scale-y-100", move || memoized_when() == true)
            class=("hidden", move || memoized_when() == false)
        >
            {children()}
        </div>
    }
}

#[component]
pub fn About() -> impl IntoView {
    let params = use_params_map();

    let section_length = 4;
    let section = move || {
        params.with(|params| {
            params
                .get("section")
                .cloned()
                .unwrap()
                .parse::<i32>()
                .unwrap()
        })
    };

    let (touch_start_y, set_touch_start_y) = create_signal(-1);

    let down_available = move || section() < section_length;
    let up_available = move || section() > 1;
    let go_to_next_section =
        move || use_navigate()(&format!("/about/{}", section() + 1), Default::default());

    let go_to_prev_section =
        move || use_navigate()(&format!("/about/{}", section() - 1), Default::default());

    let handle_scroll = move |e: WheelEvent| {
        if e.delta_y() > 0.0 && section() < section_length {
            go_to_next_section();
        } else if e.delta_y() < 0.0 && section() > 1 {
            go_to_prev_section();
        }
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
                    d if d < 0 && section() < section_length => {
                        go_to_next_section();
                        set_touch_start_y(-1);
                    }
                    d if d > 0 && section() > 1 => {
                        go_to_prev_section();
                        set_touch_start_y(-1);
                    }
                    _ => {}
                }
            }
        }
        None => set_touch_start_y(-1),
    };

    view! {
        <SourceAnchor href="#[git]"/>
        <div
            class="grow flex flex-col text-white text-lg w-1/2 max-w-xl scroll-smooth items-center space-y-10 max-w-lg"
            on:wheel=handle_scroll
            on:touchstart=handle_touch_start
            on:touchmove=handle_touch_move
        >
            <Show when=move || up_available() fallback=|| ()>
                <i
                    class="grow-0 mb-10 fas fa-chevron-up cursor-pointer"
                    on:click=move |_| go_to_prev_section()
                ></i>
            </Show>
            <div class="grow text-xl">
                <ShowWithTransition when=move || { section() == 1 }>
                    <PictureSection/>
                </ShowWithTransition>
                <ShowWithTransition when=move || { section() == 2 }>
                    <MeSection/>
                </ShowWithTransition>
                <ShowWithTransition when=move || { section() == 3 }>
                    <SiteSection/>
                </ShowWithTransition>
                <ShowWithTransition when=move || { section() == 4 }>
                    <BinaryNavSwithcer
                        a_path="skills"
                        a_display_text="Skills"
                        b_path="beliefs"
                        b_display_text="Beliefs"
                    />
                </ShowWithTransition>
                <Outlet/>
            </div>
            <Show when=move || down_available() fallback=|| ()>
                <i
                    class="grow-0 pb-20 mt-10 fas fa-chevron-down cursor-pointer"
                    on:click=move |_| go_to_next_section()
                ></i>
            </Show>

        </div>
    }
}
