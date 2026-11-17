//! Client for the Vikant Ultimate Box card reader/writer.
//!
//! Wire format and provenance: `protocol-spec/PROTOCOL.md` in this repo.
//!
//! ```no_run
//! # #[cfg(feature = "usb")] fn main() -> vikant_protocol::Result<()> {
//! use vikant_protocol::{CardKind, Client, FtdiPort};
//! let mut c = Client::connect(FtdiPort::open()?)?;
//! let size = c.identify()?.size() as usize;
//! let mut card = vec![0; size];
//! c.read(CardKind::Viking, 0, &mut card)?;
//! # Ok(()) }
//! # #[cfg(not(feature = "usb"))] fn main() {}
//! ```

pub mod card;
pub mod client;
pub mod formats;
pub mod frame;
#[cfg(feature = "usb")]
pub mod ftdi;
pub mod mock;
pub mod pattern;

pub use client::{Client, Error, Port, Result};
pub use frame::{CardKind, Dialect, FlashId};
#[cfg(feature = "usb")]
pub use ftdi::FtdiPort;
