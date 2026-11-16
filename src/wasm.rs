//! The browser boundary. Each method takes plain numbers or strings and
//! returns JSON, which keeps the JavaScript side small.

use crate::{Lab, Mode};
use serde::Serialize;
use wasm_bindgen::prelude::*;

fn json(value: &impl Serialize) -> String {
    serde_json::to_string(value).expect("lab types always serialize")
}

#[wasm_bindgen]
pub struct WasmLab {
    lab: Lab,
}

#[wasm_bindgen]
impl WasmLab {
    /// `mode` is "AP" or "CP".
    #[wasm_bindgen(constructor)]
    pub fn new(mode: &str) -> WasmLab {
        let mode = if mode == "CP" {
            Mode::Consistency
        } else {
            Mode::Availability
        };
        WasmLab {
            lab: Lab::new(mode),
        }
    }

    pub fn view(&self) -> String {
        json(&self.lab.view())
    }

    pub fn toggle_link(&mut self, a: usize, b: usize) -> String {
        json(&self.lab.toggle_link(a, b))
    }

    pub fn isolate(&mut self, i: usize) -> String {
        json(&self.lab.isolate(i))
    }

    pub fn heal(&mut self) -> String {
        json(&self.lab.heal())
    }

    pub fn sell(&mut self, i: usize, delta: i32) -> String {
        json(&self.lab.sell(i, i64::from(delta)))
    }

    pub fn set_note(&mut self, i: usize, text: &str) -> String {
        json(&self.lab.set_note(i, text))
    }

    pub fn take_lock(&mut self, i: usize) -> String {
        json(&self.lab.take_lock(i))
    }

    pub fn release_lock(&mut self, i: usize) -> String {
        json(&self.lab.release_lock(i))
    }

    pub fn expire_lease(&mut self, i: usize) -> String {
        json(&self.lab.expire_lease(i))
    }
}
