use leptos::*;

#[component]
pub fn Beliefs() -> impl IntoView {
    view! {
        <h2 class="mb-10 font-semibold">
            Evolving list of loosely held, hopefully not pretentious sounding, and strong beliefs about software engineering, mayhaps only applying to the web world:
        </h2>
        <ul class="list-none list-outside space-y-4 mb-60">
            <li>Code that feels good to write is productive code</li>
            <li>Teams should use auto-formatters</li>
            <li>
                Unit test files that have more lines of mocks than tests is a really bad sign since they cement contracts that will be null and void when business needs and implementations change
            </li>
            <li>
                Dogma is as rampant in tech as it is anywhere else people are involved. We should always stay open-minded
            </li>
            <li>r#"Everything in software is about getting things to communicate "well""#</li>
            <ul class="list-none list-outside pl-8 space-y-4">
                <li>
                    r#"To people: other developers, business stakeholders, users, API consumers"#
                </li>
                <li>
                    r#"To machines: clients to services, services to services, processes to processes"#
                </li>
                <li>
                    r#"To present-tense code: interfaces between functions, classes, network calls"#
                </li>
                <li>
                    r#"To past-tense code: gracefully handling old versions of code after deploying to prod"#
                </li>
                <li>
                    r#"To future-tense code: today's code will happily swap in and out with tomorrow's code"#
                </li>
            </ul>
            <li>
                r#"We need to remember performance is a measurement of the latency and throughput of the communication between or within machines"#
            </li>
            <li>
                r#"Performance claims that aren't measurable tend to be a result of engineers being shy about being called artists. We should not be shy"#
            </li>
            <li>Large multi-hundred-line PRs indicate problems in the process</li>
            <ul class="list-disc list-outside pl-8 space-y-4">
                <li>r#"Poor developer utilization"#</li>
                <li>
                    r#"Slower and more painful feedback cycles (requested changes have to happen in the service code and in tests)"#
                </li>
                <li>r#"Behind-the-scene iteration from single developer battling dragons"#</li>
            </ul>
        </ul>
    }
}
