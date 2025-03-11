use hpos_hal::leds::{HoloDiagnosticState, HoloLed};

fn main() {
    let l = HoloLed::new();
    l.set_state(HoloDiagnosticState::StatusOk).unwrap();
}
