mod about;
mod app;
pub mod conjunction;
mod visitors;
mod beliefs;
pub mod cluster_stats;
mod dev;
mod footer;
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
mod work;

pub use app::App;

#[cfg(feature = "ssr")]
pub fn register_server_fns() {
    use leptos::server_fn::actix::register_explicit;
    register_explicit::<photography::FetchImages>();
    register_explicit::<cluster_stats::GetClusterMetrics>();
    register_explicit::<cluster_stats::GetNodeMetrics>();
    register_explicit::<cluster_stats::GetHistoricalMetrics>();
    register_explicit::<cluster_stats::GetCephStatus>();
    register_explicit::<cluster_stats::GetNetworkInsights>();
    register_explicit::<cluster_stats::GetTopNetworkPods>();
    register_explicit::<cluster_stats::GetNetworkBreakdown>();
    register_explicit::<cluster_stats::GetSpikeConfig>();
    register_explicit::<cluster_stats::GetClaudeAuditLog>();
    register_explicit::<cluster_stats::GetGitOpsStatus>();
    register_explicit::<satellite_tracker::GetTleData>();
    register_explicit::<visitors::GetVisitorStats>();
    register_explicit::<visitors::GetMyInfo>();
    register_explicit::<visitors::ForgetMe>();
    register_explicit::<conjunction::GetConjunctionStatus>();
    register_explicit::<conjunction::GetConjunctions>();
    register_explicit::<conjunction::RetriggerConjunction>();
}
