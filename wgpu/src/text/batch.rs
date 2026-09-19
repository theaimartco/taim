use super::{Cache, Text};
use crate::core::Transformation;

pub type Batch = Vec<Item>;

#[derive(Debug)]
pub enum Item {
    Group {
        transformation: Transformation,
        text: Vec<Text>,
    },
    Cached {
        transformation: Transformation,
        cache: Cache,
    },
}
