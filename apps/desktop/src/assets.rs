//! Files embedded in the binary and served to GPUI (`img()`, `svg()`): each
//! visual style lists its own in its `ASSETS` hook (see `style.rs`).

use std::borrow::Cow;

use gpui::{AssetSource, Result, SharedString};

/// The app's asset source.
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Ok(crate::style::assets()
            .find(|(p, _)| *p == path)
            .map(|(_, bytes)| Cow::Borrowed(*bytes)))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(crate::style::assets()
            .filter(|(p, _)| p.starts_with(path))
            .map(|(p, _)| SharedString::from(*p))
            .collect())
    }
}
