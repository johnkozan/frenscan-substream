#[path = "frenscan.types.v1.rs"]
#[allow(dead_code)]
mod frenscan_priv;

pub mod frenscan {
    pub use super::frenscan_priv::*;
}
