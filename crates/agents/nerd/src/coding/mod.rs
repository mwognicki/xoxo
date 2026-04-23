pub(crate) mod noise_stripper;

pub use nerd_ast::{
    CodeLanguage, CodeStructure, CodeStructureError, FindSymbolOptions, SymbolHit,
    SymbolSearchResult, find_symbol, inspect_code_structure,
};
pub use noise_stripper::strip_noise;
