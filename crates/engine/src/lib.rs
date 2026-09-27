//! The pure core: a pack and the state of the world in, a screen tree out.

pub mod clock;
pub mod define;
pub mod engine;
pub mod expr;
mod modules;
pub mod pack;
pub mod tree;
pub mod validate;
pub mod value;
pub mod yaml;

/// The version of the screen tree this engine produces. A renderer refuses a tree it does not know.
pub const TREE_VERSION: u32 = 2;
