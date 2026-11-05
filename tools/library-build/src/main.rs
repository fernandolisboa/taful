//! Offline library build (spec §7.3). Runs on the author's PC, at low
//! priority and pausable. M0 only reserves the binary; the preflop solver
//! lands in M3 and the postflop batch in M4.

fn main() {
    eprintln!(
        "taful-library-build: nothing to build yet (library format v{}, model v{})",
        taful_solver::LIBRARY_FORMAT_VERSION,
        taful_core::MODEL_VERSION,
    );
    std::process::exit(2);
}
