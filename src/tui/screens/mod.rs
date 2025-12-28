//! TUI screens

mod address_list;
mod chain_select;
mod home;
mod keygen;
mod mnemonic;
mod reshare;
mod send;
mod wallet_details;

pub use address_list::render_address_list;
pub use chain_select::render_chain_select;
pub use home::render_home;
pub use keygen::{render_keygen, KeygenFormData};
pub use mnemonic::render_mnemonic;
pub use reshare::{render_reshare, ReshareFormData};
pub use send::{render_send, SendFormData, TxDisplay, UtxoDisplay};
pub use wallet_details::render_wallet_details;
