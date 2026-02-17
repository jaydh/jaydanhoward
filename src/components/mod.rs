mod about;
mod app;
mod beliefs;
mod cluster_stats;
mod dev;
mod icons;
mod life;
mod link;
mod nav;
mod path_search;
mod photography;
mod projects;
pub mod satellite_tracker;
#[cfg(not(feature = "ssr"))]
mod satellite_renderer;
#[cfg(not(feature = "ssr"))]
mod satellite_calculations;
mod skills;
mod source_anchor;
mod work;

pub use app::App;

#[cfg(feature = "ssr")]
pub fn register_server_fns() {
    use leptos::server_fn::actix::register_explicit;
    register_explicit::<photography::FetchImages>();
    register_explicit::<cluster_stats::GetClusterMetrics>();
    register_explicit::<cluster_stats::GetNodeMetrics>();
    register_explicit::<cluster_stats::GetHistoricalMetrics>();
    register_explicit::<satellite_tracker::GetTleData>();
}
