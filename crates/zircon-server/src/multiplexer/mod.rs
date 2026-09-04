//! Tokio TCP multiplexer: routes HTTP and Minecraft protocol traffic from the
//! public port(s) to the right backend.

pub mod detector;
pub mod disconnect;
pub mod tcp;
pub mod varint;
