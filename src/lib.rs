// pattern bytes -> ast -> compile (Glushkov NFA) -> vm (Engine)

pub mod boyermoore;
pub mod regex;

#[cfg(test)]
mod boyermoore_tests;

#[cfg(test)]
mod regex_tests;
