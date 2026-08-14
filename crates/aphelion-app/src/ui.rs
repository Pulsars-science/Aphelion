//! The egui control panel.
//!
//! One side panel with four sections: the clock, the physics dials, the display
//! dials, and the body list. The physics section is the heart of the thing —
//! those sliders are what turn a planetarium into a sandbox.

use aphelion_core::constants::{DAY, YEAR, to_au};
use aphelion_core::time::format_duration;
use aphelion_core::{BodyId, Integrator};

use crate::state::AppState;

/// Draws the whole interface into the root [`egui::Ui`].
///
/// Returns `true` if the user asked to quit.
pub fn draw(root: &mut egui::Ui, state: &mut AppState, fps: f64) -> bool {
    let mut quit = false;

    // Copied out and written back because the closure below also needs `state`
    // mutably, and egui wants the flag by &mut so that dragging the resize edge
    // past the minimum can close the panel by itself.
    let mut open = state.controls.panel_open;
    // The collapse button lives inside the panel, whose closure cannot also
    // touch `open` — egui is holding it. So it raises a flag instead.
    let mut collapse = false;

    egui::Panel::left("controls")
        .resizable(true)
        .default_size(320.0)
        // No grab handle on the collapsed panel: the button below sits in that
        // corner, and two overlapping ways to reopen it is one too many.
        .drag_to_open(false)
        .show_collapsible(root, &mut open, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("Aphelion");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .button("◀")
                            .on_hover_text("Hide the panel  (Tab)")
                            .clicked()
                        {
                            collapse = true;
                        }
                    });
                });
                ui.label(
                    egui::RichText::new(format!(
                        "{}  ·  {fps:.0} fps",
                        state.sim.epoch().to_iso8601()
                    ))
                    .monospace(),
                );
                ui.separator();

                clock_section(ui, state);
                ui.separator();
                physics_section(ui, state);
                ui.separator();
                display_section(ui, state);
                ui.separator();
                bodies_section(ui, state);
                ui.separator();

                ui.horizontal(|ui| {
                    if ui.button("Reset system").clicked() {
                        state.reset();
                    }
                    if ui.button("Quit").clicked() {
                        quit = true;
                    }
                });
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(
                        "drag: orbit · wheel: zoom · space: pause · [ ]: time · o: orbits · tab: panel",
                    )
                    .small()
                    .weak(),
                );
            });
        });

    state.controls.panel_open = open && !collapse;

    // The other half of the toggle, in the same corner the ◀ button vacated, so
    // hiding and showing read as one control rather than two.
    if !state.controls.panel_open {
        egui::Area::new("show panel".into())
            .fixed_pos(egui::pos2(8.0, 8.0))
            .show(root.ctx(), |ui| {
                if ui
                    .button("▶  Controls")
                    .on_hover_text("Show the panel  (Tab)")
                    .clicked()
                {
                    state.controls.panel_open = true;
                }
            });
    }

    quit
}

fn clock_section(ui: &mut egui::Ui, state: &mut AppState) {
    ui.horizontal(|ui| {
        let label = if state.controls.paused {
            "▶ Play"
        } else {
            "⏸ Pause"
        };
        if ui.button(label).clicked() {
            state.controls.paused = !state.controls.paused;
        }
        if ui.button("÷10").clicked() {
            state.scale_time(0.1);
        }
        if ui.button("×10").clicked() {
            state.scale_time(10.0);
        }
    });

    // Time spans nine orders of magnitude, so the slider has to be logarithmic
    // or the whole usable range collapses into the first pixel.
    ui.add(
        egui::Slider::new(&mut state.controls.time_scale, 1.0..=(100.0 * YEAR))
            .logarithmic(true)
            .text("time scale")
            .custom_formatter(|value, _| format!("{}/s", format_duration(value))),
    );

    ui.horizontal(|ui| {
        ui.label("real time:");
        ui.label(
            egui::RichText::new(format_duration(
                state.sim.epoch() - aphelion_core::Epoch::J2000,
            ))
            .monospace(),
        );
        ui.label("since J2000");
    });

    if state.last_update.throttled {
        ui.colored_label(
            egui::Color32::from_rgb(230, 160, 60),
            "⚠ step cap reached — time is running slower than the scale asks",
        );
    }
}

fn physics_section(ui: &mut egui::Ui, state: &mut AppState) {
    ui.strong("Physics");

    let mut params = *state.sim.params();
    let mut changed = false;

    changed |= ui
        .add(
            egui::Slider::new(&mut params.gravity_scale, 0.1..=10.0)
                .logarithmic(true)
                .text("gravity ×G"),
        )
        .changed();
    changed |= ui
        .add(
            egui::Slider::new(&mut params.mass_scale, 0.1..=10.0)
                .logarithmic(true)
                .text("all masses"),
        )
        .changed();
    changed |= ui
        .add(
            egui::Slider::new(&mut params.softening, 0.0..=1e9)
                .text("softening")
                .suffix(" m"),
        )
        .changed();
    changed |= ui
        .checkbox(
            &mut params.relativistic_correction,
            "relativity (1PN precession)",
        )
        .changed();

    if changed {
        state.sim.set_params(params);
    }
    if !params.is_physical() && ui.button("back to real physics").clicked() {
        params.reset_physics();
        state.sim.set_params(params);
    }

    ui.add_space(4.0);
    egui::ComboBox::from_label("integrator")
        .selected_text(state.sim.integrator.name())
        .show_ui(ui, |ui| {
            for &integrator in Integrator::ALL {
                ui.selectable_value(&mut state.sim.integrator, integrator, integrator.name());
            }
        });
    ui.add(
        egui::Slider::new(&mut state.controls.steps_per_orbit, 30.0..=5000.0)
            .logarithmic(true)
            .text("steps / orbit"),
    );

    // At low time scales one integrator step is longer than a frame's worth of
    // simulated time, so motion looks stepped. Showing the step and the time
    // still queued explains why, instead of leaving it looking like a stutter.
    let step = state.sim.suggested_timestep(state.controls.steps_per_orbit);
    ui.label(
        egui::RichText::new(format!(
            "step {} · queued {}",
            format_duration(step),
            format_duration(state.pending_time())
        ))
        .small()
        .weak(),
    );

    // The honest quality readout. A symplectic scheme oscillates around zero; a
    // steadily growing number means the step size is too coarse.
    let drift = state.sim.energy_drift();
    let (colour, verdict) = match drift.abs() {
        d if d < 1e-9 => (egui::Color32::from_rgb(120, 200, 130), "excellent"),
        d if d < 1e-6 => (egui::Color32::from_rgb(180, 200, 120), "good"),
        d if d < 1e-3 => (egui::Color32::from_rgb(230, 180, 80), "coarse"),
        _ => (egui::Color32::from_rgb(230, 110, 90), "unreliable"),
    };
    ui.horizontal(|ui| {
        ui.label("energy drift:");
        ui.colored_label(colour, format!("{drift:+.2e}  ({verdict})"));
    });
}

fn display_section(ui: &mut egui::Ui, state: &mut AppState) {
    ui.strong("Display");

    let mut scale = state.controls.radius_scale;
    if ui
        .add(
            egui::Slider::new(&mut scale, 1.0..=20_000.0)
                .logarithmic(true)
                .text("body size ×"),
        )
        .changed()
    {
        state.controls.radius_scale = scale;
        // The camera's floor is tied to the drawn radius, or zooming in would
        // put the viewer inside an exaggerated planet.
        let focus = state.controls.focus;
        state.focus_on(focus);
    }
    if ui.button("true scale").clicked() {
        state.controls.radius_scale = 1.0;
        let focus = state.controls.focus;
        state.focus_on(focus);
    }

    if ui
        .checkbox(
            &mut state.controls.clamp_body_size,
            "keep bodies inside their orbits",
        )
        .on_hover_text(
            "The Sun is 109 times the Earth's radius, so a factor large enough \
             to make the planets visible makes the Sun 4.6 AU across and hides \
             the inner system inside it. This holds each body clear of the \
             nearest orbit; the planets are unaffected.",
        )
        .changed()
    {
        let focus = state.controls.focus;
        state.focus_on(focus);
    }

    ui.checkbox(&mut state.controls.show_orbits, "orbit tracks");
    ui.checkbox(&mut state.controls.follow, "camera follows focus");
    ui.add(egui::Slider::new(&mut state.scene.ambient, 0.0..=0.4).text("night-side light"));
}

fn bodies_section(ui: &mut egui::Ui, state: &mut AppState) {
    ui.strong("Bodies");

    let names: Vec<(BodyId, String)> = state
        .sim
        .bodies()
        .iter()
        .enumerate()
        .map(|(index, body)| (BodyId(index), body.name.clone()))
        .collect();

    egui::Grid::new("body grid").num_columns(3).show(ui, |ui| {
        for (index, (id, name)) in names.iter().enumerate() {
            let selected = state.controls.focus == Some(*id);
            if ui.selectable_label(selected, name).clicked() {
                state.focus_on(Some(*id));
            }
            if index % 3 == 2 {
                ui.end_row();
            }
        }
    });

    let Some(id) = state.controls.focus else {
        return;
    };
    let Some(body) = state.sim.body(id) else {
        return;
    };

    ui.add_space(6.0);
    ui.strong(body.name.clone());
    let kind = body.kind.label();
    let radius_km = body.radius / 1000.0;
    let g = state.sim.params().gravitational_constant();
    let surface_gravity = body.surface_gravity(g);
    let escape_velocity = body.escape_velocity(g) / 1000.0;
    let mut mass = body.mass;

    egui::Grid::new("body facts").num_columns(2).show(ui, |ui| {
        ui.label("type");
        ui.label(kind);
        ui.end_row();
        ui.label("radius");
        ui.label(format!("{radius_km:.0} km"));
        ui.end_row();
        ui.label("surface gravity");
        ui.label(format!("{surface_gravity:.2} m/s²"));
        ui.end_row();
        ui.label("escape velocity");
        ui.label(format!("{escape_velocity:.2} km/s"));
        ui.end_row();

        if let Some(elements) = state.sim.elements_of(id) {
            ui.label("semi-major axis");
            ui.label(format!("{:.4} AU", to_au(elements.semi_major_axis)));
            ui.end_row();
            ui.label("eccentricity");
            ui.label(format!("{:.5}", elements.eccentricity));
            ui.end_row();
            ui.label("inclination");
            ui.label(format!("{:.3}°", elements.inclination.to_degrees()));
            ui.end_row();
            if let Some(period) = state.sim.period_of(id) {
                ui.label("period");
                ui.label(if period > 2.0 * YEAR {
                    format!("{:.3} yr", period / YEAR)
                } else {
                    format!("{:.2} d", period / DAY)
                });
                ui.end_row();
            }
        }
    });

    // Editing one body's mass is the most direct "what if" in the whole app:
    // give Jupiter ten times its mass and the inner system comes apart.
    ui.add_space(4.0);
    let reference = state.reference_mass(id);
    if ui
        .add(
            egui::Slider::new(&mut mass, reference * 0.01..=reference * 100.0)
                .logarithmic(true)
                .text("mass")
                .custom_formatter(|value, _| format!("{value:.3e} kg")),
        )
        .changed()
    {
        state.sim.set_mass(id, mass);
    }
    let ratio = mass / reference;
    if (ratio - 1.0).abs() > 1e-9 {
        ui.horizontal(|ui| {
            ui.colored_label(
                egui::Color32::from_rgb(230, 180, 80),
                format!("×{ratio:.2} of the real mass"),
            );
            if ui.small_button("restore").clicked() {
                state.sim.set_mass(id, reference);
            }
        });
    }
}
