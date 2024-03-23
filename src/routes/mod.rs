#[cfg(feature = "ssr")]
mod health_check;
#[cfg(feature = "ssr")]
mod lighthouse;
#[cfg(feature = "ssr")]
mod robots;
#[cfg(feature = "ssr")]
pub use health_check::*;
#[cfg(feature = "ssr")]
pub use lighthouse::*;
#[cfg(feature = "ssr")]
pub use robots::*;

pub fn error_chain_fmt(
    e: &impl std::error::Error,
    f: &mut std::fmt::Formatter<'_>,
) -> std::fmt::Result {
    writeln!(f, "{e}\n")?;
    let mut current = e.source();
    while let Some(cause) = current {
        writeln!(f, "Caused by:\n\t{cause}")?;
        current = cause.source();
    }
    Ok(())
}
