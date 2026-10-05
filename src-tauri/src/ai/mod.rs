//! Assistant layer: provider adapter, bounded agent loop, tools, proposals and page actions.

pub mod actions;
pub mod agent;
pub mod commands;
pub mod config;
pub mod diff;
#[cfg(test)]
pub mod eval;
pub mod provider;
pub mod proposals;
pub mod tools;

#[cfg(test)]
mod tests;
