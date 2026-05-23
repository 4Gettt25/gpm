mod cargo;
mod npm;
mod pypi_composer;

pub use cargo::CargoAdapter;
pub use npm::NpmAdapter;
pub use pypi_composer::{ComposerAdapter, PypiAdapter};
