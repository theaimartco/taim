//! Extensions Marketplace & Manager for Taim Studio.
//! Enables searching, installing, and toggling plugins and tooling.

/// Filter category for extensions view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExtensionFilter {
    #[default]
    All,
    Installed,
    Recommended,
}

impl ExtensionFilter {
    pub fn label(&self) -> &'static str {
        match self {
            ExtensionFilter::All => "All",
            ExtensionFilter::Installed => "Installed",
            ExtensionFilter::Recommended => "Recommended",
        }
    }
}

/// An extension in the marketplace.
#[derive(Debug, Clone)]
pub struct ExtensionItem {
    pub id: usize,
    pub name: String,
    pub publisher: String,
    pub version: String,
    pub description: String,
    pub downloads: String,
    pub rating: f32,
    pub is_installed: bool,
    pub is_enabled: bool,
    pub icon: &'static str,
}

/// Extensions marketplace state.
#[derive(Debug, Clone)]
pub struct ExtensionsState {
    pub query: String,
    pub filter: ExtensionFilter,
    pub extensions: Vec<ExtensionItem>,
}

impl Default for ExtensionsState {
    fn default() -> Self {
        Self::new()
    }
}

impl ExtensionsState {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            filter: ExtensionFilter::All,
            extensions: vec![
                ExtensionItem {
                    id: 1,
                    name: "rust-analyzer".to_string(),
                    publisher: "rust-lang".to_string(),
                    version: "v0.15.0".to_string(),
                    description: "Rust language support with native zero-allocation LSP"
                        .to_string(),
                    downloads: "4.8M".to_string(),
                    rating: 4.9,
                    is_installed: true,
                    is_enabled: true,
                    icon: "◈",
                },
                ExtensionItem {
                    id: 2,
                    name: "taim-highlighter".to_string(),
                    publisher: "taim".to_string(),
                    version: "v0.15.0".to_string(),
                    description: "Hardware-accelerated syntect syntax highlighter".to_string(),
                    downloads: "1.2M".to_string(),
                    rating: 5.0,
                    is_installed: true,
                    is_enabled: true,
                    icon: "⚡",
                },
                ExtensionItem {
                    id: 3,
                    name: "Even Better TOML".to_string(),
                    publisher: "tamasfe".to_string(),
                    version: "v0.19.2".to_string(),
                    description: "Fully featured TOML support with schema validation".to_string(),
                    downloads: "2.1M".to_string(),
                    rating: 4.8,
                    is_installed: true,
                    is_enabled: true,
                    icon: "▤",
                },
                ExtensionItem {
                    id: 4,
                    name: "WGSL Shader Analyzer".to_string(),
                    publisher: "wgsl-tooling".to_string(),
                    version: "v1.0.4".to_string(),
                    description: "WebGPU Shading Language syntax highlighting & validation"
                        .to_string(),
                    downloads: "450K".to_string(),
                    rating: 4.7,
                    is_installed: false,
                    is_enabled: false,
                    icon: "✦",
                },
                ExtensionItem {
                    id: 5,
                    name: "GitLens Taim Supercharged".to_string(),
                    publisher: "gitkraken".to_string(),
                    version: "v15.2.0".to_string(),
                    description: "Supercharge Git within Taim Studio with blame annotations"
                        .to_string(),
                    downloads: "3.5M".to_string(),
                    rating: 4.9,
                    is_installed: false,
                    is_enabled: false,
                    icon: "⑂",
                },
                ExtensionItem {
                    id: 6,
                    name: "GitHub Copilot Native".to_string(),
                    publisher: "github".to_string(),
                    version: "v1.88.0".to_string(),
                    description: "Your AI pair programmer integrated into Taim Studio".to_string(),
                    downloads: "8.2M".to_string(),
                    rating: 4.6,
                    is_installed: false,
                    is_enabled: false,
                    icon: "◇",
                },
            ],
        }
    }

    /// Returns items filtered by query and active category filter.
    pub fn filtered(&self) -> Vec<&ExtensionItem> {
        let q = self.query.to_lowercase();
        self.extensions
            .iter()
            .filter(|ext| match self.filter {
                ExtensionFilter::All => true,
                ExtensionFilter::Installed => ext.is_installed,
                ExtensionFilter::Recommended => !ext.is_installed,
            })
            .filter(|ext| {
                if q.is_empty() {
                    true
                } else {
                    ext.name.to_lowercase().contains(&q)
                        || ext.description.to_lowercase().contains(&q)
                        || ext.publisher.to_lowercase().contains(&q)
                }
            })
            .collect()
    }

    /// Toggles installation state of an extension.
    pub fn toggle_install(&mut self, id: usize) {
        if let Some(ext) = self.extensions.iter_mut().find(|e| e.id == id) {
            ext.is_installed = !ext.is_installed;
            if ext.is_installed {
                ext.is_enabled = true;
            }
        }
    }

    /// Toggles enabled state of an installed extension.
    pub fn toggle_enabled(&mut self, id: usize) {
        if let Some(ext) = self
            .extensions
            .iter_mut()
            .find(|e| e.id == id && e.is_installed)
        {
            ext.is_enabled = !ext.is_enabled;
        }
    }
}
