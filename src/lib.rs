extern crate lazy_static;

pub mod protocol;
pub mod message_queue;
pub mod entity;
pub mod global;
mod test;

pub use self::global::get_runtime;
