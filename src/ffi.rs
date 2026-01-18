//! FFI bindings to GNU Bash's parser
//!
//! This module includes the auto-generated bindings from bindgen.
//! The bindings are generated at build time by build.rs.

#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]
#![allow(clippy::all)]
#![allow(clippy::pedantic)]
#![allow(clippy::nursery)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
