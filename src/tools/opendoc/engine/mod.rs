//! Document editing engine.
//!
//! All editing operations work on the IR, not on native formats.
//! This means every operation works for ALL document types.

pub mod chunk;
pub mod complexity;
pub mod diff;
pub mod extract;
pub mod replace;
pub mod search;
pub mod template;
