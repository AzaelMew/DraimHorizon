// Bundled from whatlang 0.18.0 (https://github.com/greyblake/whatlang-rs/tree/v0.18.0).
// Modified only for an internal module namespace, std HashMap, disabled optional
// feature integrations and ordinary comments (no standalone-crate doctests).
// This avoids adding registry dependencies forbidden by the pinned native tool.
//
// (The MIT License)
// Copyright (c) 2017 Sergey Potapov <blake131313@gmail.com>
// Copyright (c) 2014 Titus Wormer <tituswormer@gmail.com>
// Copyright (c) 2008 Kent S Johnson
// Copyright (c) 2006 Jacob R Rideout <kde@jacobrideout.net>
// Copyright (c) 2004 Maciej Ceglowski
//
// Permission is hereby granted, free of charge, to any person obtaining
// a copy of this software and associated documentation files (the
// 'Software'), to deal in the Software without restriction, including
// without limitation the rights to use, copy, modify, merge, publish,
// distribute, sublicense, and/or sell copies of the Software, and to
// permit persons to whom the Software is furnished to do so, subject to
// the following conditions:
//
// The above copyright notice and this permission notice shall be
// included in all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED 'AS IS', WITHOUT WARRANTY OF ANY KIND,
// EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.
// IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
// CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT,
// TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE
// SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
#![allow(dead_code, unused_imports, clippy::all)]
// Whatlang is a Rust library to detect(regonize) natural languages.
// Apart from it, the library also recognizes scripts (writing system).
// Every language and script are represented by determined list of enums.
//
// # Examples
//
// Using `detect` function:
//
// ```
// use whatlang::{detect, Lang, Script};
//
// let text = "Ĉu vi ne volas eklerni Esperanton? Bonvolu! Estas unu de la plej bonaj aferoj!";
// let info = detect(text).unwrap();
// assert_eq!(info.lang(), Lang::Epo);
// assert_eq!(info.script(), Script::Latin);
//
// // Confidence is in the range from 0 to 1.
// assert_eq!(info.confidence(), 1.0);
// assert!(info.is_reliable());
// ```
//
// Using `Detector` with specified denylist or allowlist:
//
// ```
// use whatlang::{Detector, Lang};
//
// let allowlist = vec![Lang::Eng, Lang::Rus];
//
// // You can also create detector using with_denylist function
// let detector = Detector::with_allowlist(allowlist);
// let lang = detector.detect_lang("There is no reason not to learn Esperanto.");
// assert_eq!(lang, Some(Lang::Eng));
// ```
//
// # Features
//
// | Feature     | Description                                                                           |
// |-------------|---------------------------------------------------------------------------------------|
// | `enum-map`  | `Lang` and `Script` implement `Enum` trait from [enum-map](https://docs.rs/enum-map/) |
// | `arbitrary` | Support [Arbitrary](https://crates.io/crates/arbitrary)                               |
// | `serde`     | Implements `Serialize` and `Deserialize` for `Lang` and `Script`                      |
// | `dev`       | Enables `whatlang::dev` module which provides some internal API.<br/> It exists for profiling purposes and normal users are discouraged to to rely on this API.  |
//
mod alphabets;
mod combined;
mod core;
mod error;
mod lang;
mod scripts;
mod trigrams;
mod utils;

pub use crate::translation::detector::core::{Detector, Info, detect, detect_lang};
pub use crate::translation::detector::lang::Lang;
pub use crate::translation::detector::scripts::{Script, detect_script};
