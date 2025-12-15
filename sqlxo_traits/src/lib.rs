#![feature(trait_alias)]
#![feature(specialization)]
#![allow(incomplete_features)]
#![forbid(unsafe_code)]

mod core;

pub use core::*;

mod web;

pub use web::*;
