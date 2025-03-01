use crate::inventory::{HoloInventory, HoloLedDevice, HoloPlatformType, InventoryError};
/// A generic interface for handling LED state changes regardless of hardware.
use aorura as aurora;
use log::info;
use serde_derive::{Deserialize, Serialize};
use std::fs::File;
use std::io::{self, BufWriter, Write};

/// This represents the possible different states the host could be in. In general, each of these
/// states could be expected to map to an individual state for a multi-state LED, such as on the
/// Holoports, but the implementation will do as best it can with what it has. An implementation
/// using an OLED screen might display a useful diagnostic message, whereas a single LED with three
/// states (Blue, Red, Off) might choose to map Blue to [HoloDiagnosticState::StatusOk] and all
/// other states to Red. The underlying implementation will be heavily dependent on the hardware
/// capabilities.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum HoloDiagnosticState {
    /// This indicates that everything is fine.
    StatusOk,
    /// The following are each placeholders to cover all of the Holoport LED states possible. Once
    /// we determine the new set of error conditions, we'll rename this to be more descriptive
    /// symbolic names, independent of the underlying implementations.
    StatusBad1,
    StatusBad2,
    StatusBad3,
    StatusBad4,
    StatusBad5,
    StatusBad6,
    StatusBad7,
    StatusBad8,
    StatusBad9,
    StatusBad10,
    StatusBad11,
    StatusBad12,
}

/// This trait is what LED implementations will conform to in order to plug into this (simple)
/// abstract interface.
pub trait HoloLedImplementation {
    /// Sets the state of the LED or other device to represent the given [HoloDiagnosticState]
    /// value.
    fn set_state(&self, state: HoloDiagnosticState) -> Result<(), InventoryError>;
    /// This function is for convenience and just returns a string to identify this particular LED
    /// implementation.
    fn label(&self) -> String;
}

/// Simple struct to represent state saved to disk, which is written, but never read and is there
/// more to aid support and other tools.
#[derive(Debug, Serialize, Deserialize)]
pub struct LedState {
    pub implementation: String,
    pub state: HoloDiagnosticState,
}

/// This is the generic interface used to interact with any of the LED implementations. To use it,
/// have the interface discover/select the right implementation for this host, and call
/// `set_state()` on it:
/// ```no_run
/// use hpos_hal::leds::{HoloLed, HoloDiagnosticState};
/// let l = HoloLed::new();
/// l.set_state(HoloDiagnosticState::StatusOk).unwrap();
/// ```
/// Note: the above example is skipped during `cargo test` execution due to it requiring write
/// access to `/var/run`.
pub struct HoloLed {
    /// An object representing the underlying hardware LED implementation.
    implementation: Box<dyn HoloLedImplementation>,
}
impl Default for HoloLed {
    /// Default implementation to appease the clippy gods
    fn default() -> Self {
        Self::new()
    }
}

impl HoloLed {
    /// The location of the on-disk LED state file. Used to complement the actual hardware
    /// state in cases where we have LED hardware, and used instead of LED hardware where we
    /// don't. We don't really need this to persist across reboots as the state of the
    /// hardware will likely have reset in the process.
    const LED_STATE_DIR: &str = "/var/run";
    pub const LED_STATE_FILE: &str = "holo-led-state.json";

    /// Retrieves an LED implementation to use.
    pub fn new() -> Self {
        // The inventory is relatively lightweight and contains the logic for being able to
        // discover enough about the hardware to tell us what type of LED device we're dealing
        // with.
        let i = HoloInventory::from_host();
        match i.platform {
            Some(platform) => match platform.platform_type {
                HoloPlatformType::Holoport | HoloPlatformType::HoloportPlus => {
                    match platform.led_device {
                        HoloLedDevice::HoloportUsbLed { device_node } => {
                            info!("Using Holoport USB LED implementation");
                            HoloLed {
                                implementation: Box::new(HoloportUsbLed { device_node }),
                            }
                        }
                        HoloLedDevice::None() => {
                            info!("No LED device found. Using empty implementation.");
                            HoloLed {
                                implementation: Box::new(HoloLedNone {}),
                            }
                        }
                    }
                }
                _ => {
                    info!("Falling back to empty LED implementation");
                    HoloLed {
                        implementation: Box::new(HoloLedNone {}),
                    }
                }
            },
            _ => {
                info!("No platform returned from inventory. Using empty LED implementation");
                HoloLed {
                    implementation: Box::new(HoloLedNone {}),
                }
            }
        }
    }

    /// Sets the LED state.
    pub fn set_state(&self, state: HoloDiagnosticState) -> Result<(), InventoryError> {
        // We allow the default state directory to be overridden at runtime, but recommend leaving
        // this to the default LSB-compliant path. Test require this to be somewhere writable as a
        // non-root user, which is handled separately.
        let state_dir = match option_env!("LED_STATE_DIR") {
            Some(dir) => dir.to_string(),
            None => Self::LED_STATE_DIR.to_string(),
        };
        self.set_state_with_path(&state_dir, state)
    }

    /// This is a wrapper for tests to override the default (FS-global) constant default path with
    /// a path generated from something like the `tempdir` crate. This allows us to run our tests
    /// without necessarily having write access to `Self::LED_STATE_DIR`.
    #[cfg(test)]
    pub fn set_state_for_test(
        &self,
        state_dir: &str,
        state: HoloDiagnosticState,
    ) -> Result<(), InventoryError> {
        self.set_state_with_path(state_dir, state)
    }

    fn set_state_with_path(
        &self,
        path: &str,
        state: HoloDiagnosticState,
    ) -> Result<(), InventoryError> {
        // Regardless of the underlying hardware implementation, we write the state to a file in a
        // known location for supportability.
        let curr_state = LedState {
            implementation: self.implementation.label(),
            state: state.clone(),
        };
        let state_file = format!("{}/{}", path, Self::LED_STATE_FILE);
        let file = File::create(&state_file)?;
        let mut writer = BufWriter::new(file);
        serde_json::to_writer(&mut writer, &curr_state)?;
        writer.flush()?;

        self.implementation.set_state(state.clone())?;

        Ok(())
    }
}

/// This is an implementation for cases where we don't have any kind of feedback mechanism. This
/// currently (but doesn't have to) include VMs, or could include hardare with no LED support at
/// all, such as the early model/prototype Holoports. This could write to a file somewhere as a
/// fallback, but the generic implementation above already does that on our behalf.
struct HoloLedNone {}

impl HoloLedImplementation for HoloLedNone {
    fn set_state(&self, _state: HoloDiagnosticState) -> Result<(), InventoryError> {
        Ok(())
    }

    fn label(&self) -> String {
        "No hardware LED present".to_string()
    }
}

struct HoloportUsbLed {
    device_node: String,
}
impl HoloLedImplementation for HoloportUsbLed {
    fn set_state(&self, state: HoloDiagnosticState) -> Result<(), InventoryError> {
        let led = aurora::Led::open(self.device_node.clone());
        let mut led = match led {
            Ok(l) => l,
            Err(e) => {
                info!(
                    "Failed to open Holoport LED interface {}: {}",
                    &self.device_node, e
                );
                return Err(io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "Failed to open Holoport LED device",
                )
                .into());
            }
        };
        let led_state = match state {
            HoloDiagnosticState::StatusOk => aurora::State::Aurora,
            // Once we settle on potential error states, the 'StatusBad*' variant names will have
            // names that describe the type of problem encountered, and this is where we'll map
            // those conditions to LED states for the Aurora LEDs and publish documentation for
            // for end users.
            HoloDiagnosticState::StatusBad1 => aurora::State::Flash(aurora::Color::Purple),
            HoloDiagnosticState::StatusBad2 => aurora::State::Static(aurora::Color::Purple),
            HoloDiagnosticState::StatusBad3 => aurora::State::Flash(aurora::Color::Blue),
            HoloDiagnosticState::StatusBad4 => aurora::State::Static(aurora::Color::Blue),
            HoloDiagnosticState::StatusBad5 => aurora::State::Flash(aurora::Color::Red),
            HoloDiagnosticState::StatusBad6 => aurora::State::Static(aurora::Color::Red),
            HoloDiagnosticState::StatusBad7 => aurora::State::Flash(aurora::Color::Yellow),
            HoloDiagnosticState::StatusBad8 => aurora::State::Static(aurora::Color::Yellow),
            HoloDiagnosticState::StatusBad9 => aurora::State::Flash(aurora::Color::Green),
            HoloDiagnosticState::StatusBad10 => aurora::State::Static(aurora::Color::Green),
            HoloDiagnosticState::StatusBad11 => aurora::State::Flash(aurora::Color::Orange),
            HoloDiagnosticState::StatusBad12 => aurora::State::Static(aurora::Color::Orange),
        };
        match led.set(led_state) {
            Ok(_) => Ok(()),
            Err(_) => Err(io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Unable to write to Holoport LED interface",
            )
            .into()),
        }
    }

    fn label(&self) -> String {
        "Holoport USB LED".to_string()
    }
}
