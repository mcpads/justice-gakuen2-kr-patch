#[path = "shop_ui/assets.rs"]
mod assets;
#[path = "shop_ui/build.rs"]
mod build;
#[path = "shop_ui/model.rs"]
mod model;
#[path = "shop_ui/source.rs"]
mod source;

pub(crate) use build::build_shop_ui_from_source;
pub use build::{SHOP_UI_BUILD_MANIFEST_FILE, build_shop_ui};
pub use model::{ShopUiBuild, ShopUiBuildConfig, ShopUiBuildReport, ShopUiFontSources};
pub use source::SHOP_UI_PATH;

#[cfg(test)]
#[path = "shop_ui_tests.rs"]
mod tests;
