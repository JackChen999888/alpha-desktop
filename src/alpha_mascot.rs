//! Alpha's floating little spirit. Native vectors; no texture atlas or new timer.
use crate::{paint::Card, pet::PetState, pet_expressions::Expression};
use eframe::egui::Color32 as C;

const CYAN: C = C::from_rgb(104, 223, 240);
const VIOLET: C = C::from_rgb(174, 151, 246);
const INK: C = C::from_rgb(240, 245, 249);

pub(crate) fn draw(
    c: &Card,
    cx: f32,
    state: PetState,
    t: f32,
    gaze: [f32; 2],
    eyes: [[f32; 5]; 2],
    bob: f32,
    expression: Option<(Expression, f32)>,
) {
    let animated = crate::bar_layout::animations_enabled();
    let t = if animated { t } else { 0.0 };
    let bob = if animated {
        bob.clamp(-1.5, 1.0) * 0.5
    } else {
        0.0
    };
    let c = c.shifted(cx, bob);
    let accent = if state == PetState::Worried {
        crate::theme::YELLOW
    } else {
        CYAN
    };
    // A single soft pebble silhouette, a detached orbit and tiny floating hands.
    // Keep the contour readable inside the existing 32 x 30 resident slot.
    let breath = (t * 1.5).sin() * 0.25;
    c.stroke(
        -11.8,
        6.2,
        23.6,
        20.6,
        10.,
        1.5,
        C::from_rgba_unmultiplied(104, 223, 240, 25),
    );
    c.fill(
        -11.,
        7. + breath,
        22.,
        19. - breath,
        9.,
        C::from_rgb(38, 42, 50),
    );
    c.stroke(
        -11.,
        7. + breath,
        22.,
        19. - breath,
        9.,
        0.8,
        C::from_rgb(139, 158, 178),
    );
    c.path(&[(-8., 11.), (-6.7, 9.2), (-4.5, 8.5)], 1., accent);
    c.path(&[(5., 24.5), (7.2, 23.5), (8.5, 21.8)], 0.9, VIOLET);
    let orbit: Vec<_> = (0..=24)
        .map(|n| {
            let a = n as f32 / 24. * std::f32::consts::TAU;
            (6.2 * a.cos(), 3.5 + 1.5 * a.sin() - 0.12 * 6.2 * a.cos())
        })
        .collect();
    c.path(&orbit, 0.9, accent);
    let wave = if matches!(state, PetState::Curious | PetState::Happy) {
        (t * 3.).sin() * 1.4
    } else {
        0.
    };
    c.fill(-14., 17., 2.5, 4., 1.25, VIOLET);
    c.fill(11.5, 16. + wave, 2.5, 4., 1.25, accent);

    let mut e = eyes;
    let mut look = if animated {
        [gaze[0] * 0.45, gaze[1] * 0.4]
    } else {
        [0., 0.]
    };
    let mut smile = false;
    let mut blush = false;
    let mut heart = false;
    let mut sparkle = false;
    if let Some((expression, phase)) = expression {
        use Expression::*;
        let p = if animated {
            (phase * std::f32::consts::PI).sin().max(0.)
        } else {
            1.
        };
        e = [[3.8, 0., 0., 0., 0.]; 2];
        match expression {
            Wink => {
                e[1] = [3.8 * (1. - p), 2.1 * p, 0., 0., 0.];
                smile = true;
            }
            Smug => {
                e[0] = [1., 1.5, -0.5, 0., -0.3];
                e[1][0] = 2.;
                smile = true;
            }
            Blush => {
                blush = true;
                look[1] = 0.5 * p;
                smile = true;
            }
            Delighted | Praised => {
                e = [[0., 2., -1.2 * p, 0., 0.]; 2];
                smile = true;
                sparkle = expression == Praised;
            }
            SideGlance => {
                look[0] = 1.5 * p;
                e[0][0] = 2.;
            }
            Heart => {
                heart = true;
                e = [[0., 2., -0.9, 0., 0.]; 2];
            }
            Giggle => {
                e = [[0., 2., -1., 0., 0.]; 2];
                smile = true;
                look[1] = (t * 5.).sin() * 0.3;
            }
            ShyPeek => {
                blush = true;
                e[0][0] = 1. + 2.8 * p;
                e[1][0] = 1.;
                look[0] = -p;
            }
            Scan => {
                look[0] = (phase * std::f32::consts::TAU).sin() * 1.6;
                e[1][0] = 2.5;
            }
            Peek => {
                e[0][0] = 1. + 3. * p;
                e[1][0] = 1. + 2. * p;
                look[1] = 0.6 * (1. - p);
            }
            Search => {
                look[0] = (phase * std::f32::consts::TAU).sin() * 1.6;
                look[1] = -0.7 * p;
            }
        }
    }
    for (i, eye) in e.into_iter().enumerate() {
        let x = if i == 0 { -4.3 } else { 4.3 } + look[0];
        let y = 14.4 + look[1] + eye[3] * 0.4;
        let points: Vec<_> = (0..=10)
            .map(|n| {
                let q = n as f32 / 5. - 1.;
                (
                    x + q * eye[1] * 0.8,
                    y + q * (eye[0] * 0.34 + eye[4] * 0.5) + (1. - q * q) * eye[2] * 0.8,
                )
            })
            .collect();
        c.path(&points, 1.9, INK);
        for p in [points[0], points[10]] {
            c.circle(p.0, p.1, 0.95, INK);
        }
    }
    if blush {
        for x in [-8.4, 8.4] {
            c.seg((x - 0.8, 17.8), (x + 0.8, 17.8), 1.1, VIOLET);
        }
    }
    if smile || matches!(state, PetState::Happy | PetState::Satisfied) {
        c.path(&[(-2., 18.), (0., 18.9), (2., 18.)], 0.8, accent);
    }
    if heart {
        c.circle(-1.5, 22., 1.5, VIOLET);
        c.circle(1.5, 22., 1.5, VIOLET);
        c.poly(&[(-3., 22.), (0., 25.), (3., 22.)], VIOLET);
    }
    if sparkle || state == PetState::Happy {
        c.seg((12., 4.), (12., 8.), 1., accent);
        c.seg((10., 6.), (14., 6.), 1., accent);
    }
    match state {
        PetState::Music => {
            // Headphone cushions plus three restrained beat bars.
            c.path(
                &[(-13., 14.), (-13., 10.), (-9., 6.8), (-4., 6.)],
                1.,
                VIOLET,
            );
            c.path(&[(4., 6.), (9., 6.8), (13., 10.), (13., 14.)], 1., VIOLET);
            c.fill(-14., 12., 2., 6., 1., accent);
            c.fill(12., 12., 2., 6., 1., accent);
            for i in 0..3 {
                let x = (i as f32 - 1.) * 2.6;
                let h = 1.2 + ((t * 3. + i as f32 * 1.8).sin() + 1.) * 1.;
                c.seg((x, 25.), (x, 25. - h), 1.1, accent);
            }
        }
        PetState::Worried => {
            c.seg((13.5, 4.), (13.5, 7.), 1.2, accent);
            c.circle(13.5, 9.3, 0.7, accent);
        }
        PetState::Focused => {
            c.seg((-3., 24.), (3., 24.), 1.3, VIOLET);
            c.circle(0., 24., 0.8, INK);
        }
        PetState::Sleeping | PetState::Dozing => {
            c.path(&[(10., 4.), (14., 4.), (10., 7.), (14., 7.)], 0.9, VIOLET);
        }
        PetState::Satisfied if !heart => crate::paint::droplet(&c, 0., 24., 3.5, accent),
        _ => {}
    }
}

#[cfg(test)]
mod preview {
    use super::*;
    use eframe::egui::{self, Pos2, Rect, Vec2};
    #[test]
    #[ignore = "Export the actual Alpha mascot renderer for visual review"]
    fn alpha_mascot_gallery() {
        let dest = std::path::PathBuf::from(
            std::env::var_os("ALPHA_MASCOT_PREVIEW").expect("preview directory"),
        );
        std::fs::create_dir_all(&dest).unwrap();
        let scenes = [
            ("Idle", PetState::Awake),
            ("Curious", PetState::Curious),
            ("Waking", PetState::Waking),
            ("Celebrate", PetState::Happy),
            ("Hydrated", PetState::Satisfied),
            ("Music", PetState::Music),
            ("Attention", PetState::Worried),
            ("Focus", PetState::Focused),
            ("Drowsy", PetState::Dozing),
            ("Sleeping", PetState::Sleeping),
        ];
        let names = [
            "Wink",
            "Confident",
            "Blush",
            "Delighted",
            "Side glance",
            "Praised",
            "Heart",
            "Giggle",
            "Shy peek",
            "Scan",
            "Peek",
            "Search",
        ];
        for (theme, palette) in [
            ("dark", None),
            ("light", Some(crate::skin_paint::GLASS_LIGHT)),
            ("cream", Some(crate::skin_paint::CREAM)),
        ] {
            let ctx = egui::Context::default();
            let mut raster = crate::preview_raster::Raster::new(960, 400);
            for frame in 0..24 {
                let t = frame as f64 / 12.;
                let output = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(960., 400.))),
                        time: Some(t),
                        ..Default::default()
                    },
                    |ctx| {
                        egui::CentralPanel::default()
                            .frame(egui::Frame::NONE)
                            .show(ctx, |ui| {
                                let card = Card::new(ui, Pos2::ZERO, ui.max_rect());
                                for i in 0..22 {
                                    let x = (i % 6) as f32 * 160.;
                                    let y = (i / 6) as f32 * 100.;
                                    let (name, state, expression) = if i < 10 {
                                        (scenes[i].0, scenes[i].1, None)
                                    } else {
                                        (
                                            names[i - 10],
                                            PetState::Awake,
                                            Some((Expression::ALL[i - 10], frame as f32 / 23.)),
                                        )
                                    };
                                    let mut pet = crate::pet::Pet::default();
                                    for step in 0..40 {
                                        pet.step(
                                            state,
                                            t + step as f64 / 60.,
                                            1. / 60.,
                                            [0., 0.],
                                            true,
                                        );
                                    }
                                    let c = card.shifted(x + 80., y + 7.).scaled(0., 0., 2.);
                                    if let Some(expression) = expression {
                                        draw(
                                            &c,
                                            0.,
                                            state,
                                            t as f32,
                                            [0., 0.],
                                            [[4.4, 0., 0., 0., 0.]; 2],
                                            0.,
                                            Some(expression),
                                        );
                                    } else {
                                        pet.draw(&c, 0., t, palette);
                                    }
                                    card.centered_text(
                                        x + 80.,
                                        y + 78.,
                                        name,
                                        12.,
                                        crate::theme::W::Regular,
                                        INK,
                                        crate::paint::H::Center,
                                    );
                                }
                            });
                    },
                );
                raster.update_textures(&output.textures_delta);
                let mut shapes = output.shapes;
                if let Some(p) = palette {
                    for shape in &mut shapes {
                        crate::skin_paint::recolor(&mut shape.shape, &p);
                    }
                }
                let bg = if theme == "dark" {
                    [0.012, 0.014, 0.019, 1.]
                } else {
                    [0.94, 0.94, 0.94, 1.]
                };
                raster.buf.fill(bg);
                raster.draw(
                    &ctx.tessellate(shapes, output.pixels_per_point),
                    output.pixels_per_point,
                );
                let mut rgba = Vec::new();
                raster.to_rgba8(&mut rgba);
                image::RgbaImage::from_raw(960, 400, rgba)
                    .unwrap()
                    .save(dest.join(format!("{theme}-{frame:02}.png")))
                    .unwrap();
                raster.free_textures(&output.textures_delta);
            }
        }
    }
}
