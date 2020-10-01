//! # log_filter_parse
use std::{borrow::Cow, collections::HashMap};

#[derive(Debug)]
pub enum FiltersKind {
    /// A default filter (no logging)
    Default,
    /// A blanket filter (covers everything below it)
    Blanket,
    /// A list of modules to level filters
    ///     
    /// This is split split off from the Map because its generally faster to
    /// iterate over a small Vec compared to a HashMap
    List(Vec<(Cow<'static, str>, log::LevelFilter)>),
    /// A mapping of modules to level filters
    ///
    /// This is split split off from the Map because its generally faster to
    /// iterate over a small Vec compared to a HashMap
    Map(HashMap<Cow<'static, str>, log::LevelFilter>),
}

/// Parsed level filters
#[derive(Debug)]
pub struct Filters {
    /// The kin of filter
    pub kind: FiltersKind,
    /// The minimum level
    pub minimum: Option<log::LevelFilter>,
}

impl Default for Filters {
    fn default() -> Self {
        Self {
            kind: FiltersKind::Default,
            minimum: None,
        }
    }
}

impl Filters {
    #[allow(clippy::should_implement_trait)]
    /// Parses the level filters from the input str
    pub fn from_str(input: &str) -> Self {
        let mut mapping = input.split(',').filter_map(parse).collect::<Vec<_>>();

        let minimum = input
            .split(',')
            .filter(|s| !s.contains('='))
            .flat_map(|s| s.parse().ok())
            .filter(|&l| l != log::LevelFilter::Off)
            .max();

        let kind = match mapping.len() {
            0 if minimum.is_none() => FiltersKind::Default,
            0 => FiltersKind::Blanket,
            d if d < 15 => {
                mapping.shrink_to_fit();
                FiltersKind::List(mapping)
            }
            _ => FiltersKind::Map(mapping.into_iter().collect()),
        };

        Self { kind, minimum }
    }

    /// Parses the level filters from the environment variable `RUST_LOG`
    pub fn from_env() -> Self {
        std::env::var("RUST_LOG")
            .ok()
            .as_deref()
            .map(Self::from_str)
            .unwrap_or_default()
    }

    #[inline]
    /// Checks to see whether the module in the metadata has logging enabled
    pub fn is_enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        match self.find_module(metadata.target()) {
            Some(level) => metadata.level() <= level,
            None => false,
        }
    }

    #[inline]
    /// Attempts to find the specified `module` in this collection
    ///
    /// If the `FiltersKind` is `Default`, then None is returned.
    pub fn find_module(&self, module: &str) -> Option<log::LevelFilter> {
        match self.kind {
            FiltersKind::Default => return None,
            FiltersKind::Blanket => return self.minimum,
            _ => {}
        }

        if let Some(level) = self.find_exact(module) {
            return Some(level);
        }

        let mut last = false;
        for (i, ch) in module.char_indices().rev() {
            if last {
                last = false;
                if ch == ':' {
                    if let Some(level) = self.find_exact(&module[..i]) {
                        return Some(level);
                    }
                }
            } else if ch == ':' {
                last = true
            }
        }

        self.minimum
    }

    #[inline]
    fn find_exact(&self, module: &str) -> Option<log::LevelFilter> {
        match &self.kind {
            FiltersKind::Default => None,
            FiltersKind::Blanket => self.minimum,
            FiltersKind::List(levels) => {
                levels
                    .iter()
                    .find_map(|(m, level)| if m == module { Some(*level) } else { None })
            }
            FiltersKind::Map(levels) => levels.get(module).copied(),
        }
    }
}

#[inline]
fn parse(input: &str) -> Option<(Cow<'static, str>, log::LevelFilter)> {
    let mut iter = input.split('=');
    Some((
        Cow::Owned(iter.next()?.to_string()),
        iter.next()?.to_ascii_uppercase().parse().ok()?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn filters() {
        let input = "debug,foo::bar=off,foo::baz=trace,foo=info,baz=off,quux=error";
        let filters = Filters::from_str(input);

        let modules = &[
            ("foo::bar", log::LevelFilter::Off),
            ("foo::baz", log::LevelFilter::Trace),
            ("foo", log::LevelFilter::Info),
            ("baz", log::LevelFilter::Off),
            ("quux", log::LevelFilter::Error),
            ("something", log::LevelFilter::Debug),
            ("another::thing", log::LevelFilter::Debug),
        ];

        for (module, expected) in modules {
            assert_eq!(filters.find_module(module).unwrap(), *expected);
        }
    }

    #[test]
    fn minimum() {
        let filters =
            Filters::from_str("debug,foo::bar=off,foo::baz=trace,foo=info,baz=off,quux=error");

        let modules = &[
            ("foo::bar", log::LevelFilter::Off),
            ("foo::baz", log::LevelFilter::Trace),
            ("foo", log::LevelFilter::Info),
            ("baz", log::LevelFilter::Off),
            ("quux", log::LevelFilter::Error),
            ("something", log::LevelFilter::Debug),
            ("another::thing", log::LevelFilter::Debug),
            ("this::is::unknown", log::LevelFilter::Debug),
        ];

        for (module, expected) in modules {
            assert_eq!(filters.find_module(module).unwrap(), *expected);
        }
    }
}
