//! Measures how well each integrator conserves energy over a century of the
//! inner Solar System.
//!
//! ```text
//! cargo run --release -p aphelion-data --example integrator_comparison
//! ```
//!
//! Two columns matter. `worst` is the largest excursion seen at any point,
//! `final` the error left at the end. For a symplectic scheme `final` is much
//! smaller than `worst` — the error is an oscillation the integrator keeps
//! coming back from. For Runge–Kutta the two are the same, because the error is
//! a one-way drift.

use aphelion_core::Integrator;
use aphelion_core::constants::{DAY, YEAR};

const YEARS: u32 = 100;

fn main() {
    println!(
        "Inner Solar System, {YEARS} years — relative energy error |ΔE/E|\n\
         (a symplectic scheme returns from its excursions; RK4 does not)\n"
    );
    println!(
        "{:<22} {:>8} {:>6} {:>12} {:>12} {:>12}",
        "integrator", "dt", "evals", "worst", "final", "at 50 yr"
    );
    println!("{}", "-".repeat(76));

    for &integrator in Integrator::ALL {
        for step_days in [1.0, 0.25] {
            let mut sim = aphelion_data::solar_system_inner();
            sim.integrator = integrator;

            let mut worst: f64 = 0.0;
            let mut midpoint = 0.0;
            for year in 0..YEARS {
                sim.advance(YEAR, step_days * DAY);
                worst = worst.max(sim.energy_drift().abs());
                if year == YEARS / 2 - 1 {
                    midpoint = sim.energy_drift().abs();
                }
            }

            println!(
                "{:<22} {step_days:>6} d {:>6} {worst:>12.3e} {:>12.3e} {midpoint:>12.3e}",
                integrator.name(),
                integrator.force_evaluations(),
                sim.energy_drift().abs(),
            );
        }
    }
}
