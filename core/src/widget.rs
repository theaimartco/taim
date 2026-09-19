//! Create custom widgets and operate on them.
pub mod operation;
pub mod text;
pub mod tree;
pub mod visit;

mod id;

pub use id::Id;
pub use operation::Operation;
pub use text::Text;
pub use tree::Tree;
pub use visit::Visit;

mod api;

pub use api::{Void, Widget};
